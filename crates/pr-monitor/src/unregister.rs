//! `--unregister [--purge]`: asks a running instance to quit, then removes what PR Monitor added
//! to the system (and, with `--purge`, its data folder).

use std::{
    io::Write as _,
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

use prm_config::{AppPaths, Config, normalize};
use prm_domain::OrgId;
use prm_platform::{
    Acquired, HandoffRequest, InstanceScope, UnregisterSpec, acquire_instance, forward_to_primary,
};

/// How long a running instance gets to quit.
const QUIT_WAIT: Duration = Duration::from_secs(5);
const RETRY: Duration = Duration::from_millis(100);

/// Org ids from `config.toml`, read without writing anything.
fn org_ids(root: &Path) -> Vec<OrgId> {
    let paths = AppPaths::from_root(root.to_path_buf());
    let Ok(text) = std::fs::read_to_string(&paths.config) else {
        return Vec::new();
    };
    let Ok(config) = toml::from_str::<Config>(&text) else {
        return Vec::new();
    };
    let (config, _) = normalize(config);
    config
        .orgs
        .iter()
        .filter_map(|o| o.org_id().cloned())
        .collect()
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
                    let _ = forward_to_primary(&key, &HandoffRequest::Quit, Duration::from_secs(1));
                    asked = true;
                }
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(RETRY);
            }
        }
    }
}

pub(crate) fn run(root: &Path, scope: InstanceScope, purge: bool, home: Option<&Path>) -> ExitCode {
    let mut out = prm_platform::console::console_out();
    let Some((lock, suffix)) = take_lock(root, scope) else {
        let _ = writeln!(out, "PR Monitor is still running. Close it and try again.");
        return ExitCode::FAILURE;
    };
    let orgs = org_ids(root);
    let report = prm_platform::unregister(&UnregisterSpec {
        root,
        scope,
        instance_suffix: &suffix,
        purge,
        org_ids: &orgs,
        home,
    });
    drop(lock);
    let mut ok = true;
    for (step, result) in &report.steps {
        let line = match result {
            Ok(()) => format!("ok   {step}"),
            Err(error) => {
                ok = false;
                format!("FAIL {step}: {error}")
            }
        };
        let _ = writeln!(out, "{line}");
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
