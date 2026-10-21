//! Everything the UI (and the activation router) can ask the service to do.

use prm_config::{Config, UiState};
use prm_domain::{
    OrgId, PrKey, Secret, Timestamp, auth::SignInMethod, ids::RequestId, person::PersonCandidate,
    uri::AppUri,
};
use tokio::sync::mpsc::UnboundedSender;

/// `Debug` is safe to log: `Secret` redacts itself.
#[derive(Debug)]
pub enum Command {
    /// Refresh every enabled org.
    RefreshNow,
    RefreshOrg(OrgId),
    /// Fetch projects and repos for the scope editor; the result lands in `OrgStatus.catalog`.
    LoadCatalog {
        org: OrgId,
        force: bool,
    },
    /// Normalize, save and apply a whole new config.
    ApplyConfig(Box<Config>),
    SignIn {
        org: OrgId,
        method: SignInMethod,
    },
    CancelSignIn {
        org: OrgId,
    },
    SignOut {
        org: OrgId,
    },
    /// Validate the format, check it against the org, then store it.
    SetPat {
        org: OrgId,
        pat: Secret,
    },
    SearchPeople {
        org: OrgId,
        query: String,
        request: RequestId,
    },
    ResolvePerson {
        org: OrgId,
        candidate: PersonCandidate,
        request: RequestId,
    },
    /// Open the PR in the browser (allow-listed hosts only) and mark it seen.
    OpenPr(PrKey),
    /// Open an allow-listed link (docs, org pages).
    OpenUrl(url::Url),
    MarkSeen(Vec<PrKey>),
    MarkAllSeen,
    /// The UI debounces; the service persists (debounced again).
    SaveUiState(Box<UiState>),
    UiPresence {
        visible: bool,
        focused: bool,
    },
    /// `None` resumes notifications.
    SetNotificationsPaused {
        until: Option<Timestamp>,
    },
    TestNotification,
    Activate(AppUri),
    Shutdown,
}

/// Cheap to clone; never blocks.
#[derive(Clone, Debug)]
pub struct CommandSender(UnboundedSender<Command>);

impl CommandSender {
    /// Wraps the sending half of the service's command channel.
    pub fn new(tx: UnboundedSender<Command>) -> Self {
        Self(tx)
    }

    /// `false` once the service has stopped.
    pub fn send(&self, command: Command) -> bool {
        self.0.send(command).is_ok()
    }
}
