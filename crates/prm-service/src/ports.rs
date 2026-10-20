//! Ports the service defines (toasts, launch at login) plus re-exports of every other port, so
//! adapters and the bin import them from one place.

use prm_domain::notify::Notification;

pub use crate::event::EventSink;
pub use prm_ado::{AdoApi, AdoConnector, Credential, HttpTransport};
pub use prm_auth::{AuthFactory, CommandRunner, OrgAuthenticator, SecretKey, SecretStore};
pub use prm_config::StateRepo;
pub use prm_domain::ports::{Clock, PortError, UrlOpener};

/// Whether toasts can be shown right now, as the OS reports it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NotifierCapability {
    Enabled,
    /// Turned off in the OS notification settings.
    DisabledByUser,
    /// Turned off by group policy.
    DisabledByPolicy,
    Unsupported,
    #[default]
    Unknown,
}

/// Shows and retracts toasts. Called on the blocking pool, never on the UI thread.
pub trait Notifier: Send + Sync + 'static {
    fn show(&self, notification: &Notification) -> Result<(), PortError>;
    /// Removes a toast shown earlier with this tag and group.
    fn retract(&self, tag: &str, group: &str) -> Result<(), PortError>;
    fn capability(&self) -> NotifierCapability;
}

/// Launch-at-login as the OS sees it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AutostartState {
    Enabled,
    Disabled,
    /// Registered, but switched off by the user (Task Manager "Startup apps").
    DisabledByUser,
    Unsupported,
}

/// Registers the app to start at login. Called on the blocking pool.
pub trait Autostart: Send + Sync + 'static {
    fn state(&self) -> Result<AutostartState, PortError>;
    fn set_enabled(&self, enabled: bool) -> Result<(), PortError>;
}
