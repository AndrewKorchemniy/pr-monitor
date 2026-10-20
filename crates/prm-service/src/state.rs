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
    pub fn org(&self, id: &OrgId) -> Option<&OrgStatus> {
        self.orgs.iter().find(|o| &o.id == id)
    }

    pub fn row(&self, key: &PrKey) -> Option<&PrRow> {
        self.rows.iter().find(|r| &r.key == key)
    }

    /// An enabled org that failed or needs sign-in.
    pub fn any_org_broken(&self) -> bool {
        self.orgs.iter().any(OrgStatus::is_broken)
    }

    pub fn tray_badge(&self) -> TrayBadge {
        tray_badge(
            &self.summary,
            self.any_org_broken(),
            self.refresh.in_progress,
        )
    }

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

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RefreshStatus {
    pub in_progress: bool,
    pub progress: Arc<[RefreshProgress]>,
    pub last_completed: Option<Timestamp>,
    pub next_due: Option<Timestamp>,
    pub interval: Duration,
    /// Some org refreshes slower than configured because of rate limits.
    pub slowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshProgress {
    pub org: OrgId,
    pub phase: RefreshPhase,
    pub done: u32,
    pub total: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RefreshPhase {
    Authenticating,
    Catalog,
    PullRequests,
    Threads,
    Resolving,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct NotificationsStatus {
    pub enabled: bool,
    pub paused_until: Option<Timestamp>,
    pub capability: NotifierCapability,
    /// Toasts held back by the last cycle (paused, focused or disabled).
    pub last_suppressed: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct AutostartStatus {
    pub desired: bool,
    /// `None` until read from the OS.
    pub actual: Option<AutostartState>,
    pub error: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrgStatus {
    pub id: OrgId,
    pub label: Arc<str>,
    pub url: Arc<str>,
    pub enabled: bool,
    pub auth: AuthStatus,
    pub health: OrgHealth,
    pub last_ok: Option<Timestamp>,
    pub next_due: Option<Timestamp>,
    pub rate: RateHeadroom,
    pub scope: ScopeStats,
    pub scope_issues: Arc<[Arc<str>]>,
    pub counts: StatusSummary,
    pub catalog: Option<Arc<Catalog>>,
    pub last_error: Option<UiError>,
    pub pat_expires_on: Option<time::Date>,
}

impl OrgStatus {
    fn is_broken(&self) -> bool {
        self.enabled
            && (matches!(self.health, OrgHealth::Failed { .. })
                || matches!(
                    self.auth.state,
                    AuthStateView::NeedsSignIn { .. } | AuthStateView::Blocked { .. }
                ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthStatus {
    pub state: AuthStateView,
    pub method: Option<AuthMethod>,
    pub me: Option<Person>,
    pub tenant: Option<TenantInfo>,
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

#[derive(Clone, Debug, PartialEq, Default)]
pub enum OrgHealth {
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PauseReason {
    Auth,
    SignedOut,
    RateLimited,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RateHeadroom {
    /// `remaining / limit` from the last response, when known.
    pub remaining_fraction: Option<f32>,
    /// 1 = normal speed; up to `MAX_RATE_MULTIPLIER`.
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
