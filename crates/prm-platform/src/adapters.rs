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
    identity::RegistrationReport,
    shared::instance_key::InstanceScope,
    shell::DesktopShell,
};

pub struct PlatformInit<'a> {
    pub paths: &'a AppPaths,
    pub exe: &'a Path,
    pub scope: InstanceScope,
    /// From the instance key; names the isolated Run value.
    pub instance_suffix: &'a str,
    /// From `ensure_shell_identity` (decides the toast AUMID).
    pub registration: &'a RegistrationReport,
    /// From `PRMONITOR_SECRET_STORE`.
    pub secret_pref: SecretBackendPref,
    /// `--home` / `PRMONITOR_HOME`, repeated in the Run command.
    pub home_override: Option<&'a Path>,
}

pub struct PlatformAdapters {
    pub notifier: Arc<dyn Notifier>,
    pub autostart: Arc<dyn Autostart>,
    pub opener: Arc<dyn UrlOpener>,
    pub shell: Arc<dyn DesktopShell>,
    /// Concrete; the bin wraps it in `ChunkedSecretStore`.
    pub secrets: PlatformSecretStore,
}

pub fn adapters(init: &PlatformInit<'_>) -> PlatformAdapters {
    PlatformAdapters {
        notifier: notifier(init),
        autostart: autostart(init),
        opener: Arc::new(BrowserOpener),
        shell: shell(init),
        secrets: secret_store(&init.paths.root, init.secret_pref),
    }
}

#[cfg(windows)]
fn notifier(init: &PlatformInit<'_>) -> Arc<dyn Notifier> {
    use crate::{
        identity::IdentityStatus,
        win::toast::{POWERSHELL_AUMID, WinToastNotifier},
    };
    let aumid = if init.registration.toast_identity == IdentityStatus::Failed {
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
    use crate::{shared::run_command::run_value_name, win::autostart::WinAutostart};
    let autostart = WinAutostart::new(
        init.exe.to_path_buf(),
        init.home_override.map(Path::to_path_buf),
        run_value_name(init.scope, init.instance_suffix),
    );
    if let Err(error) = autostart.heal_path() {
        tracing::warn!(error = %error.message, "could not update launch at login path");
    }
    Arc::new(autostart)
}

#[cfg(not(windows))]
fn autostart(init: &PlatformInit<'_>) -> Arc<dyn Autostart> {
    match crate::unix::autostart::UnixAutostart::new(init.exe) {
        Ok(autostart) => Arc::new(autostart),
        Err(error) => {
            tracing::warn!(%error, "launch at login unavailable");
            Arc::new(crate::common::null::NullAutostart)
        }
    }
}

#[cfg(windows)]
fn shell(init: &PlatformInit<'_>) -> Arc<dyn DesktopShell> {
    Arc::new(crate::win::shell::WinShell::new(init.paths.root.clone()))
}

#[cfg(not(windows))]
fn shell(init: &PlatformInit<'_>) -> Arc<dyn DesktopShell> {
    Arc::new(crate::unix::shell::UnixShell::new(init.paths.root.clone()))
}
