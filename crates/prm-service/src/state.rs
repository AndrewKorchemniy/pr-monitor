//! `AppState`: everything the UI draws, published as one immutable snapshot through a `watch`
//! channel. Plain data; built by `publish`.

use std::{sync::Arc, time::Duration};

use prm_auth::AuthAvailability;
use prm_config::{Config, ConfigIssue, Severity, UiState};
use prm_domain::{
    OrgId, Person, PrKey, Timestamp,
    auth::{AuthMethod, SignInMethod, TenantInfo, UserAction},
    model::Catalog,
    scope::ScopeStats,
    view::{PrRow, StatusSummary, TrayBadge, tray_badge, tray_tooltip},
};

use crate::{
    errors::UiError,
    ports::{AutostartState, NotifierCapability},
};

#[derive(Clone, Debug, Default)]
pub struct AppState {
    /// Increments on every publish.
    pub generation: u64,
    /// Increments only when `rows` were rebuilt; the UI keys its view caches on it.
    pub rows_generation: u64,
    /// Merged across enabled orgs, in default order.
    pub rows: Arc<[PrRow]>,
    pub summary: StatusSummary,
    /// Config order.
    pub orgs: Arc<[OrgStatus]>,
    pub show_org_column: bool,
    pub refresh: RefreshStatus,
    pub notifications: NotificationsStatus,
    pub banners: Arc<[Banner]>,
    pub config: Arc<Config>,
    pub config_issues: Arc<[ConfigIssue]>,
    /// Read once by the UI at startup.
    pub ui_state: Arc<UiState>,
    pub autostart: AutostartStatus,
    /// No org is enabled: the UI offers onboarding.
    pub first_run: bool,
    pub shutting_down: bool,
}

impl AppState {
    /// The org's status by id, for org-scoped screens and actions.
    pub fn org(&self, id: &OrgId) -> Option<&OrgStatus> {
        self.orgs.iter().find(|o| &o.id == id)
    }

    /// The PR's row by key, for row actions that outlive a refresh.
    pub fn row(&self, key: &PrKey) -> Option<&PrRow> {
        self.rows.iter().find(|r| &r.key == key)
    }

    /// An enabled org that failed or needs sign-in.
    pub fn any_org_broken(&self) -> bool {
        self.orgs.iter().any(OrgStatus::is_broken)
    }

    /// The tray icon for this state: counts, a broken org, or a running refresh.
    pub fn tray_badge(&self) -> TrayBadge {
        tray_badge(
            &self.summary,
            self.any_org_broken(),
            self.refresh.in_progress,
        )
    }

    /// The tray icon's hover text for this state.
    pub fn tray_tooltip(&self) -> String {
        tray_tooltip(&self.summary, self.any_org_broken())
    }

    /// Overall progress of the running refresh (0..=1), when it can be known.
    pub fn refresh_fraction(&self) -> Option<f32> {
        if !self.refresh.in_progress {
            return None;
        }
        let (done, total) = self
            .refresh
            .progress
            .iter()
            .fold((0_u32, 0_u32), |(d, t), p| (d + p.done, t + p.total));
        #[expect(
            clippy::cast_precision_loss,
            reason = "progress counts are far below f32 precision"
        )]
        (total > 0).then(|| (done as f32 / total as f32).clamp(0.0, 1.0))
    }
}

/// Refreshing across all enabled orgs, for the status bar and the tray spinner.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RefreshStatus {
    /// Some enabled org is refreshing right now.
    pub in_progress: bool,
    /// One entry per org that is refreshing.
    pub progress: Arc<[RefreshProgress]>,
    /// The latest successful refresh of any enabled org.
    pub last_completed: Option<Timestamp>,
    /// The earliest scheduled refresh of an enabled, unpaused org.
    pub next_due: Option<Timestamp>,
    /// The configured interval between refreshes of one org.
    pub interval: Duration,
    /// Some org refreshes slower than configured because of rate limits.
    pub slowed: bool,
}

/// How far one org's running refresh has got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshProgress {
    pub org: OrgId,
    pub phase: RefreshPhase,
    /// Items finished within the current phase; resets when the phase changes.
    pub done: u32,
    /// Items in the current phase (0 while unknown); resets when the phase changes.
    pub total: u32,
}

/// The steps of one org's refresh, in the order they run.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RefreshPhase {
    Authenticating,
    Catalog,
    PullRequests,
    Threads,
    Resolving,
}

/// Whether toasts can be shown, and why not.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NotificationsStatus {
    /// The user's notification setting.
    pub enabled: bool,
    /// Toasts are paused until then.
    pub paused_until: Option<Timestamp>,
    /// What the OS allows.
    pub capability: NotifierCapability,
    /// Toasts held back by the last cycle (paused, focused or disabled).
    pub last_suppressed: u32,
}

/// Launch at login: what the user asked for and what the OS reports.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AutostartStatus {
    /// The `launch_at_login` setting.
    pub desired: bool,
    /// `None` until read from the OS.
    pub actual: Option<AutostartState>,
    /// The last read or write failure, for the settings screen.
    pub error: Option<Arc<str>>,
}

/// One configured org as the UI sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct OrgStatus {
    pub id: OrgId,
    pub label: Arc<str>,
    pub url: Arc<str>,
    pub enabled: bool,
    pub auth: AuthStatus,
    pub health: OrgHealth,
    /// The last successful refresh, kept across restarts.
    pub last_ok: Option<Timestamp>,
    /// The next scheduled refresh; `None` while paused, disabled, refreshing or not scheduled.
    pub next_due: Option<Timestamp>,
    pub rate: RateHeadroom,
    pub scope: ScopeStats,
    /// Scope problems, as user-facing text.
    pub scope_issues: Arc<[Arc<str>]>,
    /// Counts of this org's rows.
    pub counts: StatusSummary,
    /// Projects and repositories, for the scope editor.
    pub catalog: Option<Arc<Catalog>>,
    /// Why the last catalog load failed, until a load starts again or one succeeds.
    pub catalog_error: Option<UiError>,
    /// The last refresh or sign-in error; cleared by a successful refresh.
    pub last_error: Option<UiError>,
    /// From the profile's `pat_expires_on`, when it parses as a date.
    pub pat_expires_on: Option<time::Date>,
    /// New pull requests still to load: the thread read budget held them back on the last
    /// refresh, and the next refreshes come sooner until they are in.
    pub loading_more: u32,
}

impl OrgStatus {
    /// Enabled and waiting for the user to sign in (see [`AuthStateView::needs_sign_in`]).
    pub const fn needs_sign_in(&self) -> bool {
        self.enabled && self.auth.state.needs_sign_in()
    }

    fn is_broken(&self) -> bool {
        self.enabled
            && (matches!(self.health, OrgHealth::Failed { .. })
                || self.auth.state.problem().is_some())
    }
}

/// Who the org is signed in as, and how.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthStatus {
    pub state: AuthStateView,
    /// The current method, or the last one that worked.
    pub method: Option<AuthMethod>,
    pub me: Option<Person>,
    pub tenant: Option<TenantInfo>,
    /// Which sign-in methods this machine offers.
    pub availability: AuthAvailability,
}

/// The per-org auth state machine, as the UI sees it.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AuthStateView {
    #[default]
    Unknown,
    Checking,
    SignedIn,
    SigningIn {
        method: SignInMethod,
        since: Timestamp,
    },
    NeedsSignIn {
        error: UiError,
    },
    Blocked {
        error: UiError,
    },
    SignedOut,
    Disabled,
}

impl AuthStateView {
    /// The error of a problem state (needs sign-in or blocked), which raises a banner.
    pub const fn problem(&self) -> Option<&UiError> {
        match self {
            Self::NeedsSignIn { error } | Self::Blocked { error } => Some(error),
            Self::Unknown
            | Self::Checking
            | Self::SignedIn
            | Self::SigningIn { .. }
            | Self::SignedOut
            | Self::Disabled => None,
        }
    }

    /// No usable credential, and only the user can fix that: the sign-in prompts (tray item,
    /// empty state, palette) offer it. Checking or already signing in does not count.
    pub const fn needs_sign_in(&self) -> bool {
        match self {
            Self::NeedsSignIn { .. } | Self::Blocked { .. } | Self::SignedOut => true,
            Self::Unknown
            | Self::Checking
            | Self::SignedIn
            | Self::SigningIn { .. }
            | Self::Disabled => false,
        }
    }
}

/// What an org's refreshing is doing.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum OrgHealth {
    /// Waiting for the first refresh since startup or since it was enabled.
    #[default]
    Idle,
    Refreshing {
        since: Timestamp,
    },
    Ok,
    /// Some queries failed; their PRs are carried over as stale.
    Degraded {
        failed_queries: u32,
        detail: Arc<str>,
    },
    /// The last refresh failed; it is retried at `retry_at`.
    Failed {
        error: UiError,
        since: Timestamp,
        retry_at: Option<Timestamp>,
    },
    Paused {
        reason: PauseReason,
    },
    Disabled,
}

/// Why no refresh is scheduled for an enabled org.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PauseReason {
    /// Sign-in is needed or blocked; silent probes check whether it was fixed.
    Auth,
    /// The user signed out; nothing runs until they sign in.
    SignedOut,
    /// Throttled by the server.
    RateLimited,
}

/// Rate-limit state, for the org's status line.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RateHeadroom {
    /// `remaining / limit` from the last response, when known.
    pub remaining_fraction: Option<f32>,
    /// The interval stretch factor, in 1..=`MAX_RATE_MULTIPLIER` (1 = normal speed).
    pub multiplier: u32,
    pub throttled_until: Option<Timestamp>,
}

/// A persistent strip above the table. `id` is stable so the UI can remember dismissals:
/// `auth:<org>`, `offline`, `config`, `notify-policy`, `pat-expiry:<org>`, `autostart-disabled`.
#[derive(Clone, Debug, PartialEq)]
pub struct Banner {
    pub id: Arc<str>,
    pub severity: Severity,
    pub org: Option<OrgId>,
    pub title: Arc<str>,
    pub detail: Option<Arc<str>>,
    pub action: Option<UserAction>,
    pub secondary: Option<UserAction>,
    pub dismissible: bool,
}

#[cfg(test)]
mod tests;
