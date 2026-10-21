//! Building every platform adapter the service and UI need. Never fails: anything that cannot
//! start degrades to a null or in-memory adapter (logged).

use std::{path::Path, sync::Arc};

use prm_config::AppPaths;
use prm_domain::ports::UrlOpener;
use prm_service::ports::{Autostart, Notifier};

use crate::{
    common::{
        secret_store::{PlatformSecretStore, SecretBackendPref, secret_store},
        url_opener::BrowserOpener,
    },
    identity::IdentityStatus,
    shared::{instance_key::InstanceScope, registration::RegistrationScope},
    shell::DesktopShell,
};

/// What the bin knows at startup that the adapters depend on.
pub struct PlatformInit<'a> {
    pub paths: &'a AppPaths,
    pub exe: &'a Path,
    pub scope: InstanceScope,
    /// From the instance key; names the isolated launch-at-login entry.
    pub instance_suffix: &'a str,
    /// From `ensure_shell_identity` (decides the toast AUMID).
    pub toast_identity: IdentityStatus,
    /// The same scope given to `ensure_shell_identity`: whether this launch may take over the
    /// shared launch-at-login entry.
    pub registration_scope: RegistrationScope,
    /// From `PRMONITOR_SECRET_STORE`.
    pub secret_pref: SecretBackendPref,
    /// `--home` / `PRMONITOR_HOME`, repeated in the launch-at-login command.
    pub home_override: Option<&'a Path>,
}

/// The adapters, built once at startup and shared for the process lifetime.
pub struct PlatformAdapters {
    pub notifier: Arc<dyn Notifier>,
    pub autostart: Arc<dyn Autostart>,
    pub opener: Arc<dyn UrlOpener>,
    pub shell: Arc<dyn DesktopShell>,
    /// Concrete; the bin wraps it in `ChunkedSecretStore`.
    pub secrets: PlatformSecretStore,
}

/// Never fails; anything that cannot start degrades to a null or memory adapter (logged).
pub fn adapters(init: &PlatformInit<'_>) -> PlatformAdapters {
    PlatformAdapters {
        notifier: notifier(init),
        autostart: autostart(init),
        opener: Arc::new(BrowserOpener),
        shell: shell(),
        secrets: secret_store(&init.paths.root, init.secret_pref),
    }
}

#[cfg(windows)]
fn notifier(init: &PlatformInit<'_>) -> Arc<dyn Notifier> {
    use crate::win::toast::{POWERSHELL_AUMID, WinToastNotifier};
    let aumid = if init.toast_identity == IdentityStatus::Failed {
        tracing::warn!("toast identity not registered; toasts will show as Windows PowerShell");
        POWERSHELL_AUMID
    } else {
        prm_domain::app::AUMID
    };
    match WinToastNotifier::start(aumid) {
        Ok(notifier) => Arc::new(notifier),
        Err(error) => {
            tracing::warn!(%error, "notification thread failed to start");
            Arc::new(crate::common::null::NullNotifier)
        }
    }
}

#[cfg(not(windows))]
fn notifier(_init: &PlatformInit<'_>) -> Arc<dyn Notifier> {
    Arc::new(crate::unix::notify::NotifyRustNotifier::default())
}

#[cfg(windows)]
fn autostart(init: &PlatformInit<'_>) -> Arc<dyn Autostart> {
    use crate::{
        shared::{registration::should_heal_autostart, run_command::autostart_entry_name},
        win::autostart::WinAutostart,
    };
    let autostart = WinAutostart::new(
        init.exe.to_path_buf(),
        init.home_override.map(Path::to_path_buf),
        autostart_entry_name(init.scope, init.instance_suffix),
    );
    if should_heal_autostart(init.scope, init.registration_scope)
        && let Err(error) = autostart.heal_path()
    {
        tracing::warn!(error = %error.message, "could not update launch at login path");
    }
    Arc::new(autostart)
}

#[cfg(not(windows))]
fn autostart(init: &PlatformInit<'_>) -> Arc<dyn Autostart> {
    use crate::{shared::run_command::autostart_entry_name, unix::autostart::UnixAutostart};
    let name = autostart_entry_name(init.scope, init.instance_suffix);
    match UnixAutostart::new(init.exe, &name, init.home_override) {
        Ok(autostart) => Arc::new(autostart),
        Err(error) => {
            tracing::warn!(error = %error.message, "launch at login unavailable");
            Arc::new(crate::common::null::NullAutostart)
        }
    }
}

#[cfg(windows)]
fn shell() -> Arc<dyn DesktopShell> {
    Arc::new(crate::win::shell::WinShell::new())
}

#[cfg(not(windows))]
fn shell() -> Arc<dyn DesktopShell> {
    Arc::new(crate::unix::shell::UnixShell::new())
}
