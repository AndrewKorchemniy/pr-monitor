//! How much shell registration a launch may write, and when deleting a data folder is safe.

use std::path::Path;

use prm_domain::app::DATA_DIR_NAME;

use crate::shared::instance_key::InstanceScope;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegistrationScope {
    /// Toast identity and the URL protocol, always refreshed.
    Full,
    /// Toast identity only, and only if missing (dev builds and custom data roots must not
    /// hijack the installed app's protocol).
    ToastOnlyIfMissing,
}

/// `PRMONITOR_REGISTER` forces `Full`; dev builds and custom roots are cautious.
pub const fn registration_scope(
    default_root: bool,
    debug_build: bool,
    env_register: bool,
) -> RegistrationScope {
    if env_register || (default_root && !debug_build) {
        RegistrationScope::Full
    } else {
        RegistrationScope::ToastOnlyIfMissing
    }
}

/// Fewest path components an isolated root may have before `--purge` deletes it.
const MIN_ISOLATED_DEPTH: usize = 3;

/// `--purge` deletes only what is clearly PR Monitor's data folder.
pub fn safe_to_purge(root: &Path, scope: InstanceScope, home: Option<&Path>) -> bool {
    match scope {
        InstanceScope::Default => root.file_name().is_some_and(|name| name == DATA_DIR_NAME),
        InstanceScope::Isolated => {
            root.components().count() >= MIN_ISOLATED_DEPTH
                && home.is_none_or(|h| h != root)
                && (root.join("config.toml").is_file() || root.join("state.json").is_file())
        }
    }
}

#[cfg(test)]
mod tests;
