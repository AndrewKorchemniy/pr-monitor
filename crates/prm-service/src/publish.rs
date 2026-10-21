//! Builds `AppState` from the org runtimes (pure), derives banners, and de-duplicates notices.
//! The same code builds the very first state from `state.json`, so frame one shows cached rows.

use std::{collections::BTreeMap, sync::Arc};

use prm_config::{Config, ConfigIssue, Severity, UiState};
use prm_domain::{
    OrgId, PrKey, Timestamp,
    auth::{AuthMethod, UserAction},
    snapshot::SeenMap,
    view::{PrRow, RowBuildInput, build_rows, summarize},
};
use tokio::time::Instant;

use crate::{
    consts::{NOTICE_DEDUPE, PAT_EXPIRY_WARNING_DAYS},
    org::{OrgData, OrgLive},
    ports::{AutostartState, NotifierCapability},
    scheduler::{Moment, Policy},
    state::{
        AppState, AuthStatus, AutostartStatus, Banner, NotificationsStatus, OrgHealth, OrgStatus,
        RateHeadroom, RefreshProgress, RefreshStatus,
    },
    text,
};

/// One org's parts, borrowed from the runtime (or from the startup data).
pub(crate) struct OrgView<'a> {
    pub data: &'a OrgData,
    pub live: &'a OrgLive,
}

/// Everything `compose` reads.
pub(crate) struct Composition<'a> {
    pub config: &'a Arc<Config>,
    pub config_issues: &'a Arc<[ConfigIssue]>,
    pub orgs: &'a [OrgView<'a>],
    pub rows: Arc<[PrRow]>,
    pub rows_generation: u64,
    pub generation: u64,
    pub progress: Arc<[RefreshProgress]>,
    pub notifications: NotificationsStatus,
    pub ui_state: &'a Arc<UiState>,
    pub autostart: &'a AutostartStatus,
    pub shutting_down: bool,
    pub now: Moment,
}

pub(crate) fn compose(c: Composition<'_>) -> AppState {
    let orgs: Vec<OrgStatus> = c
        .orgs
        .iter()
        .map(|o| org_status(o, &c.rows, c.now))
        .collect();
    let enabled = c.config.enabled_orgs().count();
    let banners = banners(&BannerInput {
        orgs: &orgs,
        issues: c.config_issues,
        notifications: &c.notifications,
        autostart: c.autostart,
        today: c.now.wall.inner().date(),
    });
    AppState {
        generation: c.generation,
        rows_generation: c.rows_generation,
        summary: summarize(c.rows.iter()),
        refresh: refresh_status(c.orgs, c.progress, c.config),
        rows: c.rows,
        orgs: orgs.into(),
        show_org_column: c.config.ui.show_org_column.resolve(enabled),
        notifications: c.notifications,
        banners: banners.into(),
        config: c.config.clone(),
        config_issues: c.config_issues.clone(),
        ui_state: c.ui_state.clone(),
        autostart: c.autostart.clone(),
        first_run: enabled == 0,
        shutting_down: c.shutting_down,
    }
}

/// Rows for the enabled orgs, in default order.
pub(crate) fn rows_for(orgs: &[OrgView<'_>], seen: &SeenMap, now: Timestamp) -> Vec<PrRow> {
    let inputs: Vec<RowBuildInput<'_>> = orgs
        .iter()
        .filter(|o| o.data.profile.enabled)
        .map(|o| RowBuildInput {
            org: &o.data.id,
            org_label: &o.data.label,
            snapshot: &o.data.snapshot,
            records: o.data.records.as_ref(),
            reviewers: &o.data.classify.reviewers,
            ignored: &o.data.classify.ignored_comment_authors,
            org_stale: o.live.rows_stale(),
        })
        .collect();
    build_rows(&inputs, seen, now)
}

fn org_status(view: &OrgView<'_>, rows: &[PrRow], now: Moment) -> OrgStatus {
    let (data, live) = (view.data, view.live);
    let schedule = &live.schedule;
    OrgStatus {
        id: data.id.clone(),
        label: data.label.clone(),
        url: Arc::from(data.profile.url.as_str()),
        enabled: data.profile.enabled,
        auth: AuthStatus {
            state: live.auth_state.view(),
            method: live.auth.method.or(data.last_good_method),
            me: data.me.clone(),
            tenant: live.auth.tenant.or(data.tenant.map(|t| t.info)),
            availability: live.auth.availability,
        },
        health: live.health.clone(),
        last_ok: data.last_ok,
        next_due: schedule.due_wall().filter(|_| schedule.paused.is_none()),
        rate: RateHeadroom {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a 0..=1 fraction for display"
            )]
            remaining_fraction: live.rate.headroom().map(|h| h as f32),
            multiplier: schedule.multiplier,
            throttled_until: live.rate.throttled_until.map(|at| now.wall_of(at)),
        },
        scope: data.scope_stats,
        scope_issues: data.scope_issues.iter().map(text::scope_issue).collect(),
        counts: summarize(rows.iter().filter(|r| r.key.org == data.id)),
        catalog: data.catalog.clone(),
        last_error: live.last_error.clone(),
        pat_expires_on: parse_date(&data.profile.pat_expires_on),
    }
}

fn parse_date(text: &str) -> Option<time::Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    time::Date::parse(text.trim(), format).ok()
}

fn refresh_status(
    orgs: &[OrgView<'_>],
    progress: Arc<[RefreshProgress]>,
    config: &Config,
) -> RefreshStatus {
    let enabled = || orgs.iter().filter(|o| o.data.profile.enabled);
    RefreshStatus {
        in_progress: enabled().any(|o| matches!(o.live.health, OrgHealth::Refreshing { .. })),
        progress,
        last_completed: enabled().filter_map(|o| o.data.last_ok).max(),
        next_due: enabled()
            .filter(|o| o.live.schedule.paused.is_none())
            .filter_map(|o| o.live.schedule.due_wall())
            .min(),
        interval: Policy::from_config(&config.refresh).interval,
        slowed: enabled().any(|o| o.live.schedule.multiplier > 1),
    }
}

/// What the banners are derived from.
pub(crate) struct BannerInput<'a> {
    pub orgs: &'a [OrgStatus],
    pub issues: &'a [ConfigIssue],
    pub notifications: &'a NotificationsStatus,
    pub autostart: &'a AutostartStatus,
    pub today: time::Date,
}

/// The persistent strips above the table, in display order.
pub(crate) fn banners(input: &BannerInput<'_>) -> Vec<Banner> {
    let enabled: Vec<&OrgStatus> = input.orgs.iter().filter(|o| o.enabled).collect();
    let mut out: Vec<Banner> = enabled.iter().filter_map(|o| auth_banner(o)).collect();
    out.extend(enabled.iter().filter_map(|o| pat_banner(o, input.today)));
    let offline = !enabled.is_empty()
        && enabled.iter().all(|o| match &o.health {
            OrgHealth::Failed { error, .. } => error.transient,
            OrgHealth::Idle
            | OrgHealth::Refreshing { .. }
            | OrgHealth::Ok
            | OrgHealth::Degraded { .. }
            | OrgHealth::Paused { .. }
            | OrgHealth::Disabled => false,
        });
    if offline {
        out.push(banner("offline", Severity::Warning, None, text::OFFLINE));
    }
    if input.issues.iter().any(|i| i.severity == Severity::Error) {
        let mut config = banner("config", Severity::Error, None, text::CONFIG_ERRORS);
        config.action = Some(UserAction::OpenSettings(
            prm_domain::auth::SettingsSection::General,
        ));
        out.push(config);
    }
    out.extend(notify_banner(input.notifications));
    let autostart_off =
        input.autostart.desired && input.autostart.actual == Some(AutostartState::DisabledByUser);
    if autostart_off {
        out.push(banner(
            "autostart-disabled",
            Severity::Info,
            None,
            text::AUTOSTART_DISABLED,
        ));
    }
    out
}

fn banner(id: &str, severity: Severity, org: Option<&OrgId>, title: &str) -> Banner {
    Banner {
        id: Arc::from(id),
        severity,
        org: org.cloned(),
        title: Arc::from(title),
        detail: None,
        action: None,
        secondary: None,
        dismissible: true,
    }
}

fn auth_banner(org: &OrgStatus) -> Option<Banner> {
    let error = org.auth.state.problem()?;
    Some(Banner {
        detail: Some(error.title.clone()),
        action: error.action.clone(),
        secondary: error.secondary.clone(),
        dismissible: false,
        ..banner(
            &format!("auth:{}", org.id),
            Severity::Error,
            Some(&org.id),
            &text::sign_in_needed(&org.label),
        )
    })
}

fn pat_banner(org: &OrgStatus, today: time::Date) -> Option<Banner> {
    if org.auth.method != Some(AuthMethod::Pat) {
        return None;
    }
    let days = (org.pat_expires_on? - today).whole_days();
    if days > PAT_EXPIRY_WARNING_DAYS {
        return None;
    }
    Some(Banner {
        action: Some(UserAction::EnterPat),
        ..banner(
            &format!("pat-expiry:{}", org.id),
            Severity::Warning,
            Some(&org.id),
            &text::pat_expiry(&org.label, days),
        )
    })
}

fn notify_banner(notifications: &NotificationsStatus) -> Option<Banner> {
    if !notifications.enabled {
        return None;
    }
    let title = match notifications.capability {
        NotifierCapability::DisabledByUser => text::NOTIFY_BLOCKED_BY_USER,
        NotifierCapability::DisabledByPolicy => text::NOTIFY_BLOCKED_BY_POLICY,
        NotifierCapability::Enabled
        | NotifierCapability::Unsupported
        | NotifierCapability::Unknown => return None,
    };
    Some(banner("notify-policy", Severity::Warning, None, title))
}

/// Remembers recent notices so the same text for the same org shows once per window.
#[derive(Default)]
pub(crate) struct NoticeLog {
    recent: BTreeMap<(Arc<str>, Option<OrgId>), Instant>,
}

impl NoticeLog {
    /// `true` if the notice should be shown now (and records it).
    pub(crate) fn admit(&mut self, text: &Arc<str>, org: Option<&OrgId>, now: Instant) -> bool {
        self.recent
            .retain(|_, at| now.saturating_duration_since(*at) < NOTICE_DEDUPE);
        let key = (text.clone(), org.cloned());
        if self.recent.contains_key(&key) {
            return false;
        }
        self.recent.insert(key, now);
        true
    }
}

/// Seen marks for keys that still exist (in a snapshot or a tombstone).
pub(crate) fn prune_seen<'a>(
    seen: &SeenMap,
    orgs: impl Iterator<Item = &'a OrgData> + Clone,
) -> SeenMap {
    seen.iter()
        .filter(|(key, _)| orgs.clone().any(|o| o.id == key.org && o.knows(key)))
        .map(|(key, fp)| (key.clone(), *fp))
        .collect()
}

/// The fingerprints of the given keys, as currently classified.
pub(crate) fn fingerprints<'a>(
    keys: impl IntoIterator<Item = &'a PrKey>,
    orgs: &[&OrgData],
) -> SeenMap {
    keys.into_iter()
        .filter_map(|key| {
            let org = orgs.iter().find(|o| o.id == key.org)?;
            let classified = org.snapshot.get(key)?;
            Some((key.clone(), classified.fingerprint))
        })
        .collect()
}

#[cfg(test)]
mod tests;
