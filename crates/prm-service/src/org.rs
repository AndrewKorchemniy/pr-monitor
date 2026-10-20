//! One org at run time, in three parts: `OrgData` (what is persisted), `OrgLive` (what is
//! published), and `OrgLink` (the authenticator, API client and running jobs).

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use prm_ado::{AdoApi, AdoConnector, ClientPolicy, OrgEndpoint, RateSnapshot};
use prm_auth::{AuthFactory, AuthSnapshot, AuthSpec, OrgAuthenticator, entra::EntraConfig};
use prm_config::{
    OrgPersisted, OrgProfile,
    state::{ProbeCache, TenantCache},
};
use prm_domain::{
    ClassifyConfig, OrgId, Person, PrKey, Timestamp,
    auth::{AuthMethod, TenantInfo},
    diff::{ChangeEvent, DiffInput, diff},
    ids::RequestId,
    model::{Catalog, PrRecord},
    person::PeopleCache,
    plan::PrQuery,
    scope::{ScopeIssue, ScopeStats},
    snapshot::{OrgSnapshot, classify_record},
    tombstone::{CloseOutcome, Tombstones, apply_tombstones},
};
use tokio::{task::AbortHandle, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    auth_machine::AuthState,
    consts::{PEOPLE_CACHE_CAP, PEOPLE_PERSIST_CAP},
    errors::UiError,
    scheduler::Schedule,
    state::{OrgHealth, PauseReason},
};

/// Everything about an org that survives a restart, plus what is derived from it.
pub(crate) struct OrgData {
    pub id: OrgId,
    pub profile: OrgProfile,
    pub label: Arc<str>,
    pub classify: ClassifyConfig,
    pub me: Option<Person>,
    pub tenant: Option<TenantCache>,
    pub catalog: Option<Arc<Catalog>>,
    pub records: Arc<BTreeMap<PrKey, PrRecord>>,
    pub stale: BTreeSet<PrKey>,
    pub snapshot: OrgSnapshot,
    pub tombstones: Tombstones,
    /// A snapshot exists to diff against; the first refresh without one is silent.
    pub baseline: bool,
    /// Scope or authors changed: the next refresh reports no `NewPr` events.
    pub rebaseline_pending: bool,
    pub people: PeopleCache,
    pub org_wide: Option<ProbeCache>,
    pub signed_out: bool,
    pub last_good_method: Option<AuthMethod>,
    pub last_ok: Option<Timestamp>,
    pub scope_stats: ScopeStats,
    pub scope_issues: Vec<ScopeIssue>,
}

/// What one successful refresh fetched, ready to absorb.
pub(crate) struct Fetched {
    pub me: Person,
    pub tenant: Option<TenantInfo>,
    /// `Some` only when refetched this time.
    pub catalog: Option<Arc<Catalog>>,
    pub scope_stats: ScopeStats,
    pub scope_issues: Vec<ScopeIssue>,
    pub records: BTreeMap<PrKey, PrRecord>,
    pub stale: BTreeSet<PrKey>,
    pub closed: Vec<(PrKey, CloseOutcome)>,
    pub harvested: Vec<Person>,
    pub failed_queries: Vec<PrQuery>,
    pub org_wide: Option<ProbeCache>,
}

pub(crate) struct Absorbed {
    pub events: Vec<ChangeEvent>,
    /// This was the org's first refresh ever: everything counts as already seen.
    pub first_baseline: bool,
}

impl OrgData {
    pub(crate) fn new(
        id: OrgId,
        profile: OrgProfile,
        persisted: Option<OrgPersisted>,
        catalog: Option<Catalog>,
        now: Timestamp,
    ) -> Self {
        let saved = persisted.unwrap_or_default();
        let mut data = Self {
            label: profile.display_label(),
            classify: profile.classify_config(),
            id,
            profile,
            me: saved.me,
            tenant: saved.tenant,
            catalog: catalog.map(Arc::new),
            records: Arc::new(saved.records),
            stale: BTreeSet::new(),
            snapshot: OrgSnapshot::new(),
            tombstones: saved.tombstones,
            baseline: saved.baseline,
            rebaseline_pending: false,
            people: PeopleCache::from_persisted(saved.people, now),
            org_wide: saved.org_wide_probe,
            signed_out: saved.signed_out,
            last_good_method: saved.last_good_method,
            last_ok: saved.last_refresh_ok,
            scope_stats: ScopeStats::default(),
            scope_issues: Vec::new(),
        };
        data.harvest_configured(now);
        data.reclassify();
        data
    }

    /// Takes a new profile for the same org; labels and rules follow.
    pub(crate) fn set_profile(&mut self, profile: OrgProfile, now: Timestamp) {
        if profile.tenant_override() != self.profile.tenant_override() {
            // A tenant saved while an override was set may be the override: rediscover.
            self.tenant = None;
        }
        self.label = profile.display_label();
        self.classify = profile.classify_config();
        self.profile = profile;
        self.harvest_configured(now);
    }

    /// Configured people are always findable in the local picker.
    fn harvest_configured(&mut self, now: Timestamp) {
        let people = &self.profile.people;
        let configured = people
            .tracked_authors
            .iter()
            .chain(&people.reviewers)
            .chain(&people.ignored_comment_authors)
            .cloned();
        self.people.harvest(configured, now);
    }

    /// Rebuilds the snapshot from the stored records with the current rules (no events).
    pub(crate) fn reclassify(&mut self) {
        self.snapshot = self.classify_all();
    }

    fn classify_all(&self) -> OrgSnapshot {
        self.records
            .iter()
            .map(|(key, record)| {
                let stale = self.stale.contains(key);
                (key.clone(), classify_record(record, &self.classify, stale))
            })
            .collect()
    }

    /// Applies a successful refresh and returns its change events (none on the baseline).
    pub(crate) fn absorb(&mut self, fetched: Fetched, now: Timestamp) -> Absorbed {
        self.me = Some(fetched.me);
        // why: with an override the auth tenant *is* the override; saving it as discovered
        // would keep it in use for `TENANT_TTL` after the override is removed.
        if let Some(info) = fetched.tenant
            && self.profile.tenant_override().is_none()
            && self.tenant.is_none_or(|t| t.info != info)
        {
            self.tenant = Some(TenantCache {
                info,
                discovered_at: now,
            });
        }
        if let Some(catalog) = fetched.catalog {
            self.catalog = Some(catalog);
        }
        self.people.harvest(fetched.harvested, now);
        self.people.evict_to(PEOPLE_CACHE_CAP);
        self.records = Arc::new(fetched.records);
        self.stale = fetched.stale;
        self.scope_stats = fetched.scope_stats;
        self.scope_issues = fetched.scope_issues;
        self.org_wide = fetched.org_wide;
        self.last_ok = Some(now);
        let next = self.classify_all();
        let events = self.diff_against(&next, &fetched.closed, now);
        let first_baseline = !self.baseline;
        self.baseline = true;
        self.rebaseline_pending = false;
        apply_tombstones(&mut self.tombstones, &events, now);
        self.snapshot = next;
        Absorbed {
            events,
            first_baseline,
        }
    }

    fn diff_against(
        &self,
        next: &OrgSnapshot,
        closed: &[(PrKey, CloseOutcome)],
        now: Timestamp,
    ) -> Vec<ChangeEvent> {
        diff(&DiffInput {
            prev: self.baseline.then_some(&self.snapshot),
            next,
            closed,
            tombstones: &self.tombstones,
            me: self.me.as_ref().map(|m| m.id),
            ignored: &self.classify.ignored_comment_authors,
            suppress_new: self.rebaseline_pending,
            now,
        })
    }

    pub(crate) fn to_persisted(&self, method: Option<AuthMethod>) -> OrgPersisted {
        OrgPersisted {
            tenant: self.tenant,
            me: self.me.clone(),
            last_good_method: method.or(self.last_good_method),
            signed_out: self.signed_out,
            baseline: self.baseline,
            last_refresh_ok: self.last_ok,
            org_wide_probe: self.org_wide,
            records: (*self.records).clone(),
            tombstones: self.tombstones.clone(),
            people: self.people.to_persisted(PEOPLE_PERSIST_CAP),
        }
    }

    /// The PR is in the snapshot or was recently closed (so its seen mark is still useful).
    pub(crate) fn knows(&self, key: &PrKey) -> bool {
        self.snapshot.contains_key(key) || self.tombstones.contains_key(key)
    }
}

/// What the UI shows about an org besides its data.
pub(crate) struct OrgLive {
    pub auth_state: AuthState,
    pub auth: AuthSnapshot,
    pub health: OrgHealth,
    pub last_error: Option<UiError>,
    pub rate: RateSnapshot,
    pub schedule: Schedule,
}

impl OrgLive {
    /// Before any refresh: disabled, signed out, or waiting for the first refresh.
    pub(crate) fn initial(data: &OrgData) -> Self {
        let mut live = Self {
            auth_state: AuthState::Unknown,
            auth: AuthSnapshot::default(),
            health: OrgHealth::Idle,
            last_error: None,
            rate: RateSnapshot::default(),
            schedule: Schedule::default(),
        };
        if !data.profile.enabled {
            live.auth_state = AuthState::Disabled;
            live.health = OrgHealth::Disabled;
        } else if data.signed_out {
            live.auth_state = AuthState::SignedOut;
            live.health = OrgHealth::Paused {
                reason: PauseReason::SignedOut,
            };
            live.schedule.paused = Some(PauseReason::SignedOut);
        }
        live
    }

    /// The org's rows are shown as out of date.
    pub(crate) const fn rows_stale(&self) -> bool {
        matches!(
            self.health,
            OrgHealth::Failed { .. } | OrgHealth::Paused { .. }
        )
    }
}

/// The org's running refresh or probe; at most one at a time.
pub(crate) struct InFlight {
    pub cancel: CancellationToken,
}

/// The org's connections and the handles of its running jobs.
pub(crate) struct OrgLink {
    pub endpoint: OrgEndpoint,
    pub auth: Arc<dyn OrgAuthenticator>,
    pub api: Arc<dyn AdoApi>,
    /// Bumped whenever results of running jobs must be ignored (config change, sign-out).
    pub generation: u64,
    pub in_flight: Option<InFlight>,
    /// The notification cycle the running refresh belongs to.
    pub cycle: Option<u64>,
    pub sign_in: Option<CancellationToken>,
    /// The auth state to restore when a sign-in is cancelled.
    pub pre_sign_in: Option<AuthState>,
    pub search: Option<(RequestId, AbortHandle)>,
    pub catalog_loading: bool,
    pub last_probe: Option<Instant>,
    pub last_manual: Option<Instant>,
}

/// What `OrgLink::connect` needs from the service.
pub(crate) struct Connectors<'a> {
    pub auth: &'a dyn AuthFactory,
    pub ado: &'a dyn AdoConnector,
    pub policy: &'a ClientPolicy,
}

impl OrgLink {
    pub(crate) fn connect(data: &OrgData, with: &Connectors<'_>) -> Self {
        let endpoint = OrgEndpoint::new(data.id.clone());
        let (auth, api) = Self::clients(data, &endpoint, with);
        Self {
            endpoint,
            auth,
            api,
            generation: 0,
            in_flight: None,
            cycle: None,
            sign_in: None,
            pre_sign_in: None,
            search: None,
            catalog_loading: false,
            last_probe: None,
            last_manual: None,
        }
    }

    /// New authenticator and API client after the endpoint or auth settings changed.
    pub(crate) fn reconnect(&mut self, data: &OrgData, with: &Connectors<'_>) {
        let (auth, api) = Self::clients(data, &self.endpoint, with);
        self.auth = auth;
        self.api = api;
        self.generation += 1;
    }

    fn clients(
        data: &OrgData,
        endpoint: &OrgEndpoint,
        with: &Connectors<'_>,
    ) -> (Arc<dyn OrgAuthenticator>, Arc<dyn AdoApi>) {
        let auth = with.auth.create(auth_spec(data, endpoint));
        let credential: Arc<dyn prm_ado::Credential> = auth.clone();
        let api = with
            .ado
            .connect(endpoint.clone(), credential, with.policy.clone());
        (auth, api)
    }

    /// Cancels the running refresh or probe; its result will be ignored.
    pub(crate) fn cancel_in_flight(&mut self) -> bool {
        let Some(job) = self.in_flight.take() else {
            return false;
        };
        job.cancel.cancel();
        self.generation += 1;
        true
    }

    pub(crate) fn cancel_sign_in(&mut self) {
        if let Some(token) = self.sign_in.take() {
            token.cancel();
        }
    }

    pub(crate) fn abort_search(&mut self) {
        if let Some((_, handle)) = self.search.take() {
            handle.abort();
        }
    }
}

fn auth_spec(data: &OrgData, endpoint: &OrgEndpoint) -> AuthSpec {
    let profile = &data.profile;
    AuthSpec {
        org: data.id.clone(),
        endpoint: endpoint.clone(),
        mode: profile.auth,
        tenant_override: profile.tenant_override(),
        tenant_cached: data.tenant.map(|t| (t.info, t.discovered_at)),
        entra: entra_config(profile),
        preferred: data.last_good_method,
        signed_out: data.signed_out,
    }
}

/// Advanced sign-in is configured only with a valid client id; never pre-filled.
fn entra_config(profile: &OrgProfile) -> Option<EntraConfig> {
    let client_id = uuid::Uuid::parse_str(profile.entra.client_id.trim()).ok()?;
    let tenant = profile.entra.tenant.trim();
    Some(EntraConfig {
        client_id,
        tenant: (!tenant.is_empty()).then(|| tenant.to_owned()),
    })
}

/// An org at run time.
pub(crate) struct OrgRuntime {
    pub data: OrgData,
    pub live: OrgLive,
    pub link: OrgLink,
}

#[cfg(test)]
mod tests;
