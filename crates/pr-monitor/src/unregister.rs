//! `--unregister [--purge]`: asks a running instance to quit, then removes what PR Monitor added
//! to the system (and, with `--purge`, its data folders).

use std::{
    io::Write as _,
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

use prm_domain::{OrgId, app::APP_DISPLAY_NAME};
use prm_platform::{
    Acquired, HandoffRequest, InstanceScope, UnregisterSpec, acquire_instance, forward_to_primary,
};

use crate::{
    report::print_results,
    root::{LOCK_RETRY, Root},
};

/// How long a running instance gets to quit.
const QUIT_WAIT: Duration = Duration::from_secs(5);
/// How long the running instance gets to accept the quit request.
const QUIT_FORWARD_BUDGET: Duration = Duration::from_secs(1);

/// Org ids from `config.toml` (migrated in memory), read without writing anything.
fn org_ids(root: &Root) -> Vec<OrgId> {
    prm_config::read_config(&root.app_paths())
        .map(|config| config.org_ids())
        .unwrap_or_default()
}

/// Holds the instance lock (asking a running primary to quit first).
fn take_lock(root: &Path, scope: InstanceScope) -> Option<(prm_platform::InstanceLock, String)> {
    let deadline = Instant::now() + QUIT_WAIT;
    let mut asked = false;
    loop {
        match acquire_instance(root, scope).ok()? {
            (Acquired::Primary(lock), key) => return Some((lock, key.suffix)),
            (Acquired::Secondary, key) => {
                if !asked {
                    let _ = forward_to_primary(&key, &HandoffRequest::Quit, QUIT_FORWARD_BUDGET);
                    asked = true;
                }
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(LOCK_RETRY);
            }
        }
    }
}

/// `user_home` is the user's profile folder, which `--purge` never deletes (not the `--home`
/// override the data root came from: comparing a root with itself let `--home .` delete the
/// profile).
pub(crate) fn run(root: &Root, purge: bool, user_home: Option<&Path>) -> ExitCode {
    let mut out = prm_platform::console::console_out();
    let scope = root.scope;
    let Some((lock, suffix)) = take_lock(&root.path, scope) else {
        let _ = writeln!(
            out,
            "{APP_DISPLAY_NAME} is still running. Close it and try again."
        );
        return ExitCode::FAILURE;
    };
    let orgs = org_ids(root);
    let report = prm_platform::unregister(&UnregisterSpec {
        root: &root.path,
        config_root: &root.config_root,
        legacy_root: root.legacy.as_deref(),
        scope,
        instance_suffix: &suffix,
        purge,
        org_ids: &orgs,
        user_home,
    });
    drop(lock);
    let steps = report
        .steps
        .iter()
        .map(|(step, result)| (*step, result.as_ref().copied().map_err(String::as_str)));
    if print_results(&mut out, steps) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
