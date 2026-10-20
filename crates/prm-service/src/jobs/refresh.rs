//! One org's refresh: authenticate, list PRs, read threads, resolve vanished PRs. Touches no
//! service state; returns everything for the actor to apply.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use futures_util::{StreamExt, stream};
use prm_ado::{AdoApi, AdoError, OrgWideProbe, RateSnapshot};
use prm_auth::{AuthSnapshot, OrgAuthenticator};
use prm_config::{OrgProfile, state::ProbeCache};
use prm_domain::{
    OrgId, Person, PrKey, Timestamp,
    auth::AuthError,
    model::{Catalog, PrRecord, PrState, PullRequest, ThreadDigest},
    plan::{PrQuery, QueryMode, TrackSpec, plan_queries, select_tracked},
    ports::Clock,
    scope::{ResolvedScope, resolve_scope},
    tombstone::CloseOutcome,
};
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    consts::{
        CATALOG_TTL, LIST_CONCURRENCY, ORG_WIDE_PROBE_TTL, REFRESH_DEADLINE, THREAD_CONCURRENCY,
        VANISHED_RESOLVE_CAP,
    },
    org::Fetched,
    state::{RefreshPhase, RefreshProgress},
};

/// Everything a refresh reads; cloned out of the org runtime when the job starts.
pub(crate) struct RefreshCtx {
    pub org: OrgId,
    pub api: Arc<dyn AdoApi>,
    pub auth: Arc<dyn OrgAuthenticator>,
    pub profile: OrgProfile,
    pub me: Option<Person>,
    pub catalog: Option<Arc<Catalog>>,
    pub clock: Arc<dyn Clock>,
    pub prev: Arc<BTreeMap<PrKey, PrRecord>>,
    pub org_wide: Option<ProbeCache>,
    pub progress: UnboundedSender<RefreshProgress>,
}

pub(crate) struct RefreshOutput {
    pub fetched: Fetched,
    pub auth: AuthSnapshot,
    pub rate: RateSnapshot,
    pub requests: u64,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub(crate) enum RefreshFailure {
    Auth(AuthError),
    Ado(AdoError),
    Timeout,
    Cancelled,
}

impl From<AdoError> for RefreshFailure {
    fn from(error: AdoError) -> Self {
        if let AdoError::Auth(auth) = &error {
            return Self::Auth(auth.clone());
        }
        Self::Ado(error)
    }
}

/// Runs the refresh with the overall deadline; cancellation wins over everything.
pub(crate) async fn run(
    ctx: RefreshCtx,
    cancel: CancellationToken,
) -> Result<Box<RefreshOutput>, RefreshFailure> {
    let guarded = tokio::time::timeout(REFRESH_DEADLINE, refresh(ctx));
    match cancel.run_until_cancelled(guarded).await {
        None => Err(RefreshFailure::Cancelled),
        Some(Err(_elapsed)) => Err(RefreshFailure::Timeout),
        Some(Ok(result)) => result.map(Box::new),
    }
}

impl RefreshCtx {
    fn report(&self, phase: RefreshPhase, done: usize, total: usize) {
        let progress = RefreshProgress {
            org: self.org.clone(),
            phase,
            done: count(done),
            total: count(total),
        };
        // why: the actor owns the receiver for its whole life; a send can only fail at shutdown.
        let _ = self.progress.send(progress);
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

async fn refresh(ctx: RefreshCtx) -> Result<RefreshOutput, RefreshFailure> {
    let started = Instant::now();
    let requests_before = ctx.api.request_count();
    ctx.report(RefreshPhase::Authenticating, 0, 0);
    ctx.auth
        .access_token(None)
        .await
        .map_err(RefreshFailure::Auth)?;
    let me = who_am_i(&ctx).await?;
    let (catalog, refetched) = load_catalog(&ctx).await?;
    let scope = resolve_scope(&ctx.profile.scope, &catalog);
    let org_wide = org_wide_probe(&ctx, &catalog).await;
    let mode = match org_wide {
        Some(probe) if probe.ok && ctx.profile.org_wide_query => QueryMode::OrgWide,
        _ => QueryMode::PerProject,
    };
    let listed = list_prs(&ctx, &plan_queries(&scope, mode)).await?;
    let track = ctx.profile.track_spec(Some(&me));
    let mut harvested = harvest_prs(&listed.prs);
    let mut gathered = fetch_threads(&ctx, select_tracked(listed.prs, &scope, &track)).await;
    harvested.append(&mut gathered.commenters);
    let lost = Lost {
        scope: &scope,
        failed: &listed.failed,
        track: &track,
    };
    let closed = resolve_vanished(&ctx, &lost, &mut gathered).await;
    carry_over_failed(&ctx.prev, &listed.failed, &mut gathered);
    let mut scope_stats = scope.stats;
    scope_stats.repos_failed = failed_repo_count(&scope, &listed.failed);
    let fetched = Fetched {
        me,
        tenant: ctx.auth.snapshot().tenant,
        catalog: refetched.then_some(catalog),
        scope_stats,
        scope_issues: scope.issues,
        records: gathered.records,
        stale: gathered.stale,
        closed,
        harvested,
        failed_queries: listed.failed.into_iter().map(|(q, _)| q).collect(),
        org_wide,
    };
    Ok(RefreshOutput {
        fetched,
        auth: ctx.auth.snapshot(),
        rate: ctx.api.rate(),
        requests: ctx.api.request_count().saturating_sub(requests_before),
        elapsed: started.elapsed(),
    })
}

async fn who_am_i(ctx: &RefreshCtx) -> Result<Person, RefreshFailure> {
    if let Some(me) = &ctx.me {
        return Ok(me.clone());
    }
    Ok(ctx.api.connection_data().await?)
}

/// The cached catalog while fresh; otherwise refetched (`true`).
async fn load_catalog(ctx: &RefreshCtx) -> Result<(Arc<Catalog>, bool), RefreshFailure> {
    let now = ctx.clock.now();
    if let Some(catalog) = ctx
        .catalog
        .as_ref()
        .filter(|c| c.is_fresh(now, CATALOG_TTL))
    {
        return Ok((catalog.clone(), false));
    }
    ctx.report(RefreshPhase::Catalog, 0, 1);
    let catalog = fetch_catalog(ctx.api.as_ref(), now).await?;
    Ok((Arc::new(catalog), true))
}

/// Projects and repositories, fetched together.
pub(crate) async fn fetch_catalog(api: &dyn AdoApi, now: Timestamp) -> Result<Catalog, AdoError> {
    let (projects, repos) = tokio::try_join!(api.projects(), api.repositories())?;
    Ok(Catalog {
        fetched_at: now,
        projects,
        repos,
    })
}

/// The org-wide query is used only when opted in and a probe passed within the last day.
async fn org_wide_probe(ctx: &RefreshCtx, catalog: &Catalog) -> Option<ProbeCache> {
    if !ctx.profile.org_wide_query {
        return ctx.org_wide;
    }
    let now = ctx.clock.now();
    if let Some(cached) = ctx
        .org_wide
        .filter(|p| p.at <= now && now.since(p.at) < ORG_WIDE_PROBE_TTL)
    {
        return Some(cached);
    }
    let probe = ctx.api.probe_org_wide(catalog.projects.len()).await;
    Some(ProbeCache {
        ok: matches!(probe, Ok(OrgWideProbe::Supported)),
        at: now,
    })
}

struct Listed {
    prs: Vec<PullRequest>,
    failed: Vec<(PrQuery, AdoError)>,
}

/// Every query in parallel (bounded). Fails only if every query failed.
async fn list_prs(ctx: &RefreshCtx, queries: &[PrQuery]) -> Result<Listed, RefreshFailure> {
    ctx.report(RefreshPhase::PullRequests, 0, queries.len());
    let mut results = stream::iter(queries.iter().cloned())
        .map(|query| {
            let api = ctx.api.clone();
            async move {
                let result = api.active_pull_requests(&query).await;
                (query, result)
            }
        })
        .buffer_unordered(LIST_CONCURRENCY);
    let mut prs = BTreeMap::new();
    let mut failed = Vec::new();
    let mut done = 0;
    while let Some((query, result)) = results.next().await {
        done += 1;
        ctx.report(RefreshPhase::PullRequests, done, queries.len());
        match result {
            Ok(list) => prs.extend(list.into_iter().map(|pr| (pr.key.clone(), pr))),
            Err(error) => failed.push((query, error)),
        }
    }
    if !queries.is_empty() && failed.len() == queries.len() {
        let (_, first) = failed.swap_remove(0);
        return Err(first.into());
    }
    Ok(Listed {
        prs: prs.into_values().collect(),
        failed,
    })
}

/// Authors and reviewers (with the names ADO sent) for the local people picker.
fn harvest_prs(prs: &[PullRequest]) -> Vec<Person> {
    prs.iter()
        .flat_map(|pr| std::iter::once(&pr.author).chain(pr.reviewers.iter().map(|r| &r.who)))
        .cloned()
        .collect()
}

#[derive(Default)]
struct Gathered {
    records: BTreeMap<PrKey, PrRecord>,
    stale: BTreeSet<PrKey>,
    commenters: Vec<Person>,
}

impl Gathered {
    /// Keeps the previous record, marked stale (it produces no events).
    fn carry(&mut self, record: &PrRecord) {
        let key = record.pr.key.clone();
        self.stale.insert(key.clone());
        self.records.insert(key, record.clone());
    }
}

/// Threads per tracked PR. A failed fetch reuses the previous threads and marks the PR stale.
async fn fetch_threads(ctx: &RefreshCtx, prs: Vec<PullRequest>) -> Gathered {
    let total = prs.len();
    let now = ctx.clock.now();
    ctx.report(RefreshPhase::Threads, 0, total);
    let mut results = stream::iter(prs)
        .map(|pr| {
            let api = ctx.api.clone();
            async move {
                let result = api.threads(pr.project, pr.key.repo, pr.key.number).await;
                (pr, result)
            }
        })
        .buffer_unordered(THREAD_CONCURRENCY);
    let mut out = Gathered::default();
    let mut done = 0;
    while let Some((pr, result)) = results.next().await {
        done += 1;
        ctx.report(RefreshPhase::Threads, done, total);
        let key = pr.key.clone();
        let threads = result.unwrap_or_else(|error| {
            tracing::debug!(org = %ctx.org, pr = %key, %error, "threads failed; reusing previous");
            out.stale.insert(key.clone());
            previous_threads(&ctx.prev, &key)
        });
        out.commenters.extend(commenters(&threads));
        let record = PrRecord {
            pr,
            threads,
            fetched_at: now,
        };
        out.records.insert(key, record);
    }
    out
}

fn previous_threads(prev: &BTreeMap<PrKey, PrRecord>, key: &PrKey) -> Vec<ThreadDigest> {
    prev.get(key).map(|r| r.threads.clone()).unwrap_or_default()
}

fn commenters(threads: &[ThreadDigest]) -> impl Iterator<Item = Person> + '_ {
    threads
        .iter()
        .flat_map(|t| t.comments.iter().filter_map(|c| c.author.clone()))
}

/// Why a previously listed PR may be missing now.
struct Lost<'a> {
    scope: &'a ResolvedScope,
    failed: &'a [(PrQuery, AdoError)],
    track: &'a TrackSpec,
}

impl Lost<'_> {
    fn covered_by_failure(&self, pr: &PullRequest) -> bool {
        self.failed
            .iter()
            .any(|(q, _)| q.covers(pr.project, pr.key.repo))
    }

    /// Gone from a successful listing of a repo in scope, and its author is still tracked.
    fn vanished(&self, record: &PrRecord, listed: &BTreeMap<PrKey, PrRecord>) -> bool {
        let pr = &record.pr;
        !listed.contains_key(&pr.key)
            && self.scope.contains_repo(pr.project, pr.key.repo)
            && !self.covered_by_failure(pr)
            && self.track.authors.contains(&pr.author.id)
    }
}

/// Looks vanished PRs up by id: completed or abandoned become `closed`; lookups that fail (or
/// exceed the cap) keep the previous record as stale.
async fn resolve_vanished(
    ctx: &RefreshCtx,
    lost: &Lost<'_>,
    gathered: &mut Gathered,
) -> Vec<(PrKey, CloseOutcome)> {
    let candidates: Vec<&PrRecord> = ctx
        .prev
        .values()
        .filter(|r| lost.vanished(r, &gathered.records))
        .collect();
    let total = candidates.len().min(VANISHED_RESOLVE_CAP);
    ctx.report(RefreshPhase::Resolving, 0, total);
    let mut closed = Vec::new();
    for (index, record) in candidates.into_iter().enumerate() {
        if index >= VANISHED_RESOLVE_CAP {
            gathered.carry(record);
            continue;
        }
        let pr = &record.pr;
        match ctx
            .api
            .pull_request(pr.project, pr.key.repo, pr.key.number)
            .await
        {
            Ok(latest) => closed.extend(close_outcome(latest.state).map(|o| (pr.key.clone(), o))),
            Err(AdoError::NotFound { .. }) => closed.push((pr.key.clone(), CloseOutcome::Unknown)),
            Err(_) => gathered.carry(record),
        }
        ctx.report(RefreshPhase::Resolving, index + 1, total);
    }
    closed
}

/// `None` for a PR that is still active (hidden by a filter, not closed).
const fn close_outcome(state: PrState) -> Option<CloseOutcome> {
    match state {
        PrState::Active => None,
        PrState::Completed => Some(CloseOutcome::Completed),
        PrState::Abandoned => Some(CloseOutcome::Abandoned),
        PrState::Other => Some(CloseOutcome::Unknown),
    }
}

/// PRs whose query failed keep their previous record, marked stale.
fn carry_over_failed(
    prev: &BTreeMap<PrKey, PrRecord>,
    failed: &[(PrQuery, AdoError)],
    gathered: &mut Gathered,
) {
    for record in prev.values() {
        let pr = &record.pr;
        let covered = failed
            .iter()
            .any(|(q, _)| q.covers(pr.project, pr.key.repo));
        if covered && !gathered.records.contains_key(&pr.key) {
            gathered.carry(record);
        }
    }
}

fn failed_repo_count(scope: &ResolvedScope, failed: &[(PrQuery, AdoError)]) -> u32 {
    let failed_repos = scope
        .projects
        .iter()
        .flat_map(|(project, sel)| sel.repos.iter().map(move |repo| (*project, *repo)))
        .filter(|(project, repo)| failed.iter().any(|(q, _)| q.covers(*project, *repo)))
        .count();
    count(failed_repos)
}
