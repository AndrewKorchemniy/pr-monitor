//! `--unregister [--purge]`: removes everything PR Monitor added to the system, in an order
//! that is safe to re-run. Missing things count as removed.

use std::path::Path;

use prm_domain::ids::OrgId;

use crate::shared::{instance_key::InstanceScope, registration::safe_to_purge};

pub struct UnregisterSpec<'a> {
    pub root: &'a Path,
    pub scope: InstanceScope,
    pub instance_suffix: &'a str,
    /// Also delete the data folder (only when it is clearly ours).
    pub purge: bool,
    /// Orgs whose secrets to delete where the store cannot be enumerated (mac/Linux).
    pub org_ids: &'a [OrgId],
    pub home: Option<&'a Path>,
}

/// One entry per step, in order.
#[derive(Clone, Debug, Default)]
pub struct UnregisterReport {
    pub steps: Vec<(&'static str, Result<(), String>)>,
}

pub fn unregister(spec: &UnregisterSpec<'_>) -> UnregisterReport {
    let mut report = UnregisterReport::default();
    platform_steps(spec, &mut report);
    if spec.purge {
        let purged = if safe_to_purge(spec.root, spec.scope, spec.home) {
            match std::fs::remove_dir_all(spec.root) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        } else {
            Err("refusing to delete a folder that does not look like PR Monitor's".into())
        };
        report.steps.push(("data folder", purged));
    }
    report
}

#[cfg(windows)]
fn platform_steps(spec: &UnregisterSpec<'_>, report: &mut UnregisterReport) {
    use crate::{
        shared::run_command::run_value_name,
        win::{autostart::WinAutostart, credman, registration, toast::WinToastNotifier},
    };
    let autostart = WinAutostart::new(
        std::path::PathBuf::new(),
        None,
        run_value_name(spec.scope, spec.instance_suffix),
    );
    let message = |e: prm_domain::ports::PortError| e.message.to_string();
    report
        .steps
        .push(("launch at login", autostart.remove_all().map_err(message)));
    let cleared = WinToastNotifier::start(prm_domain::app::AUMID)
        .map_err(|e| e.to_string())
        .and_then(|notifier| notifier.clear().map_err(message));
    report.steps.push(("notifications", cleared));
    if spec.scope == InstanceScope::Default {
        report
            .steps
            .push(("URL protocol", registration::remove_protocol()));
    }
    report
        .steps
        .push(("toast identity", registration::remove_toast_identity()));
    let secrets = credman::credman_targets(prm_domain::app::CREDMAN_TARGET_PREFIX)
        .and_then(|targets| credman::delete_targets(&targets))
        .map_err(message)
        .and_then(|failures| {
            if failures == 0 {
                Ok(())
            } else {
                Err(format!("{failures} credentials could not be deleted"))
            }
        });
    report.steps.push(("saved credentials", secrets));
}

#[cfg(not(windows))]
fn platform_steps(spec: &UnregisterSpec<'_>, report: &mut UnregisterReport) {
    use prm_auth::{ChunkedSecretStore, SecretKey, SecretStore};
    use prm_service::ports::Autostart;
    let autostart = std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|exe| crate::unix::autostart::UnixAutostart::new(&exe))
        .and_then(|a| a.set_enabled(false).map_err(|e| e.message.to_string()));
    report.steps.push(("launch at login", autostart));
    let store = ChunkedSecretStore::new(crate::common::secret_store::secret_store(
        spec.root,
        crate::common::secret_store::SecretBackendPref::Auto,
    ));
    let mut result = Ok(());
    for org in spec.org_ids {
        for key in [SecretKey::pat(org), SecretKey::entra_refresh(org)] {
            if let Err(error) = store.delete(&key) {
                result = Err(error.to_string());
            }
        }
    }
    report.steps.push(("saved credentials", result));
}
