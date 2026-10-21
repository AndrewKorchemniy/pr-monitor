//! `--unregister [--purge]`: removes what PR Monitor added to the system, in an order that is
//! safe to re-run. Missing things count as removed. An isolated data root removes only its own
//! entries: the toast identity, notifications, URL protocol and saved credentials belong to
//! every instance on the account, so only the default instance removes them.

use std::path::Path;

use prm_domain::ids::OrgId;

use crate::shared::{
    instance_key::InstanceScope, registration::safe_to_purge, run_command::autostart_entry_name,
};

/// What to remove, built by the bin while it holds the instance lock.
pub struct UnregisterSpec<'a> {
    pub root: &'a Path,
    pub scope: InstanceScope,
    pub instance_suffix: &'a str,
    /// Also delete the data folder (only when it is clearly ours).
    pub purge: bool,
    /// Orgs whose secrets to delete where the store cannot be enumerated (mac/Linux).
    pub org_ids: &'a [OrgId],
    /// The user's home directory, never the `--home` override: `--purge` refuses to delete it
    /// or any folder that contains it.
    pub user_home: Option<&'a Path>,
}

/// One entry per step, in the order they ran; the text is shown to the user as is.
#[derive(Clone, Debug, Default)]
pub struct UnregisterReport {
    pub steps: Vec<(&'static str, Result<(), String>)>,
}

/// One thing `--unregister` removes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "some steps exist only on Windows")
)]
enum Step {
    LaunchAtLogin,
    Notifications,
    UrlProtocol,
    ToastIdentity,
    SavedCredentials,
    /// DPAPI-sealed secrets under this data root (`PRMONITOR_SECRET_STORE=dpapi`).
    SecretFiles,
}

impl Step {
    /// The name printed next to the step's result.
    const fn label(self) -> &'static str {
        match self {
            Self::LaunchAtLogin => "launch at login",
            Self::Notifications => "notifications",
            Self::UrlProtocol => "URL protocol",
            Self::ToastIdentity => "toast identity",
            Self::SavedCredentials => "saved credentials",
            Self::SecretFiles => "saved credential files",
        }
    }

    /// Keyed by the app rather than the data root, so every instance on the account uses it.
    const fn is_shared(self) -> bool {
        match self {
            Self::LaunchAtLogin | Self::SecretFiles => false,
            Self::Notifications
            | Self::UrlProtocol
            | Self::ToastIdentity
            | Self::SavedCredentials => true,
        }
    }
}

/// Everything this OS can remove, in order.
#[cfg(windows)]
const PLATFORM_STEPS: [Step; 6] = [
    Step::LaunchAtLogin,
    Step::Notifications,
    Step::UrlProtocol,
    Step::ToastIdentity,
    Step::SavedCredentials,
    Step::SecretFiles,
];
#[cfg(not(windows))]
const PLATFORM_STEPS: [Step; 2] = [Step::LaunchAtLogin, Step::SavedCredentials];

/// The steps for a scope: all of them for the default instance, the unshared ones otherwise.
fn plan(scope: InstanceScope) -> Vec<Step> {
    PLATFORM_STEPS
        .into_iter()
        .filter(|step| scope == InstanceScope::Default || !step.is_shared())
        .collect()
}

/// Runs every planned step (a failure does not stop the rest), then the purge if asked.
pub fn unregister(spec: &UnregisterSpec<'_>) -> UnregisterReport {
    let mut steps: Vec<_> = plan(spec.scope)
        .into_iter()
        .map(|step| (step.label(), run_step(step, spec)))
        .collect();
    if spec.purge {
        steps.push(("data folder", purge(spec)));
    }
    UnregisterReport { steps }
}

fn purge(spec: &UnregisterSpec<'_>) -> Result<(), String> {
    if !safe_to_purge(spec.root, spec.scope, spec.user_home) {
        return Err("refusing to delete a folder that does not look like PR Monitor's".into());
    }
    match std::fs::remove_dir_all(spec.root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(windows)]
fn run_step(step: Step, spec: &UnregisterSpec<'_>) -> Result<(), String> {
    use crate::win::{autostart, registration};
    let name = autostart_entry_name(spec.scope, spec.instance_suffix);
    match step {
        Step::LaunchAtLogin => autostart::remove_run_values(&name).map_err(message),
        Step::Notifications => clear_notifications(),
        Step::UrlProtocol => registration::remove_protocol(),
        Step::ToastIdentity => registration::remove_toast_identity(),
        Step::SavedCredentials => delete_credman_targets(),
        Step::SecretFiles => remove_secret_files(
            &spec
                .root
                .join(crate::shared::secret_names::SECRETS_DIR_NAME),
        ),
    }
}

#[cfg(windows)]
fn message(error: prm_domain::ports::PortError) -> String {
    error.message.to_string()
}

#[cfg(windows)]
fn clear_notifications() -> Result<(), String> {
    crate::win::toast::WinToastNotifier::start(prm_domain::app::AUMID)
        .map_err(|e| e.to_string())
        .and_then(|notifier| notifier.clear().map_err(message))
}

#[cfg(windows)]
fn delete_credman_targets() -> Result<(), String> {
    use crate::win::credman;
    let failures = credman::credman_targets(prm_domain::app::CREDMAN_TARGET_PREFIX)
        .and_then(|targets| credman::delete_targets(&targets))
        .map_err(message)?;
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} credentials could not be deleted"))
    }
}

/// Deletes our DPAPI files in `dir` (only names `dpapi_file_name` makes), then the folder if
/// that left it empty. A missing folder counts as removed.
#[cfg(windows)]
fn remove_secret_files(dir: &Path) -> Result<(), String> {
    use crate::shared::secret_names::is_dpapi_file_name;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let mut result = Ok(());
    for entry in entries.flatten() {
        let ours = entry.file_name().to_str().is_some_and(is_dpapi_file_name);
        if ours && let Err(error) = std::fs::remove_file(entry.path()) {
            result = Err(error.to_string());
        }
    }
    // why: a folder that still holds something we did not write is left alone.
    let _ = std::fs::remove_dir(dir);
    result
}

#[cfg(not(windows))]
fn run_step(step: Step, spec: &UnregisterSpec<'_>) -> Result<(), String> {
    match step {
        Step::LaunchAtLogin => remove_launch_entry(spec),
        Step::SavedCredentials => delete_org_secrets(spec),
        // why: Windows-only; PLATFORM_STEPS never plans them here.
        Step::Notifications | Step::UrlProtocol | Step::ToastIdentity | Step::SecretFiles => Ok(()),
    }
}

#[cfg(not(windows))]
fn remove_launch_entry(spec: &UnregisterSpec<'_>) -> Result<(), String> {
    use prm_service::ports::Autostart;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = autostart_entry_name(spec.scope, spec.instance_suffix);
    crate::unix::autostart::UnixAutostart::new(&exe, &name, None)
        .and_then(|autostart| autostart.set_enabled(false))
        .map_err(|e| e.message.to_string())
}

#[cfg(not(windows))]
fn delete_org_secrets(spec: &UnregisterSpec<'_>) -> Result<(), String> {
    use crate::common::secret_store::{SecretBackendPref, secret_store};
    use prm_auth::{ChunkedSecretStore, SecretKey, SecretStore};
    let store = ChunkedSecretStore::new(secret_store(spec.root, SecretBackendPref::Auto));
    let mut result = Ok(());
    for org in spec.org_ids {
        for key in SecretKey::all_for(org) {
            if let Err(error) = store.delete(&key) {
                result = Err(error.to_string());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
