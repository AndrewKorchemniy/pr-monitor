//! OS integration behind the service and UI ports: toasts, single instance and activation handoff,
//! launch at login, secrets, the proxy decision, window chrome and shell registration.
//! `unsafe` lives only in `win/`, one Win32 call per block.

mod adapters;
mod common;
pub mod console;
mod identity;
mod instance;
pub mod notify;
pub mod process;
pub mod proxy;
pub mod shared;
mod shell;
mod unregister;

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod win;

pub use adapters::{PlatformAdapters, PlatformInit, adapters};
pub use common::{
    handoff_client::{ForwardOutcome, forward_to_primary},
    handoff_server::{ActivationSink, HandoffServer},
    secret_store::{PlatformSecretStore, SecretBackendPref},
};
pub use identity::{IdentityStatus, RegistrationSpec, ensure_shell_identity};
pub use instance::{Acquired, InstanceLock, acquire_instance};
pub use shared::{
    colors::{Rgb, TitleBarColors},
    handoff_codec::HandoffRequest,
    instance_key::{InstanceKey, InstanceScope, is_same_root},
    registration::{BuildKind, RegisterOverride, RegistrationScope, registration_scope},
    session_kind::{SessionInfo, SessionKind},
};
pub use shell::DesktopShell;
pub use unregister::{UnregisterReport, UnregisterSpec, unregister};
