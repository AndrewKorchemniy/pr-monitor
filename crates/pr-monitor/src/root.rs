//! The data root: where this process keeps its files and which single-instance scope it belongs to.

use std::path::{Path, PathBuf};

use prm_config::{AppPaths, Roots};
use prm_domain::app::{ENV_HOME, LEGACY_DATA_DIR_NAME};
use prm_platform::{InstanceScope, is_same_root};

use crate::cli::Args;

/// Between attempts at the instance lock while another process still holds it.
pub(crate) const LOCK_RETRY: std::time::Duration = std::time::Duration::from_millis(100);

/// The data folder and whether it is the default one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Root {
    /// The local folder: state, cache, logs, instance lock.
    pub(crate) path: PathBuf,
    /// Where `config.toml`, `keymap.toml` and `themes` live (equal to `path` for `--home`).
    pub(crate) config_root: PathBuf,
    pub(crate) scope: InstanceScope,
    /// The resolved absolute root when it is not the default one (from `--home` or
    /// `PRMONITOR_HOME`), repeated in the launch-at-login command.
    pub(crate) home: Option<PathBuf>,
    /// The old single data folder, for the default roots only.
    pub(crate) legacy: Option<PathBuf>,
}

impl Root {
    /// `resolved` holds the absolute roots; `explicit` when they came from `--home` or
    /// `PRMONITOR_HOME`. An explicit root that names the default folder, however spelled, is the
    /// default instance under the default spelling, so one folder never gets two instance locks.
    pub(super) fn new(resolved: Roots, explicit: bool, default: Option<Roots>) -> Self {
        let home = explicit.then_some(resolved.local.as_path());
        let default_local = default.as_ref().map(|d| d.local.as_path());
        match root_scope(home, &resolved.local, default_local) {
            InstanceScope::Default => {
                let roots = default.unwrap_or(resolved);
                Self {
                    path: roots.local,
                    config_root: roots.config,
                    scope: InstanceScope::Default,
                    home: None,
                    legacy: None,
                }
            }
            InstanceScope::Isolated => Self {
                home: Some(resolved.local.clone()),
                path: resolved.local,
                config_root: resolved.config,
                scope: InstanceScope::Isolated,
                legacy: None,
            },
        }
    }

    /// The files of this root: settings under `config_root`, the rest under `path`.
    pub(crate) fn app_paths(&self) -> AppPaths {
        AppPaths::split(self.config_root.clone(), self.path.clone())
    }
}

pub(crate) fn data_root(args: &Args) -> Result<Root, prm_config::ConfigError> {
    let env_home = std::env::var_os(ENV_HOME).filter(|v| !v.is_empty());
    let explicit = args.home.is_some() || env_home.is_some();
    let cwd = std::env::current_dir().unwrap_or_default();
    let (data_local, config_dir) = (dirs::data_local_dir(), dirs::config_dir());
    // With no override, resolve_root names the default folders; that rule lives there.
    let default_roots =
        prm_config::resolve_root(None, None, data_local.clone(), config_dir.clone(), &cwd).ok();
    let legacy = data_local.as_ref().map(|l| l.join(LEGACY_DATA_DIR_NAME));
    let resolved =
        prm_config::resolve_root(args.home.clone(), env_home, data_local, config_dir, &cwd)?;
    let mut root = Root::new(resolved, explicit, default_roots);
    if root.scope == InstanceScope::Default {
        root.legacy = legacy;
    }
    Ok(root)
}

/// An override (`home`) that names the default folder, however spelled, is still the default
/// instance, so it shares the default instance's single-instance names instead of running
/// beside it on the same files.
pub(crate) fn root_scope(
    home: Option<&Path>,
    root: &Path,
    default_root: Option<&Path>,
) -> InstanceScope {
    let is_default = home.is_none() || default_root.is_some_and(|d| is_same_root(root, d));
    if is_default {
        InstanceScope::Default
    } else {
        InstanceScope::Isolated
    }
}

#[cfg(test)]
mod tests;
