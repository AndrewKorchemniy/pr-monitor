//! How much shell registration a launch may write, whether it may rewrite the launch-at-login
//! entry, and when deleting a data folder is safe.

use std::path::{Path, PathBuf};

use prm_domain::app::DATA_DIR_NAME;

use crate::shared::instance_key::{InstanceScope, normalize_root};

/// How much of the shared shell identity a launch may write.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegistrationScope {
    /// Toast identity and the URL protocol, always refreshed.
    Full,
    /// Toast identity only, and only if missing (dev builds and custom data roots must not
    /// hijack the installed app's protocol).
    ToastOnlyIfMissing,
}

/// Which kind of build is running.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildKind {
    Release,
    /// `debug_assertions` on: a developer's build, never the installed app.
    Debug,
}

impl BuildKind {
    /// The kind of this binary.
    pub const fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }
}

/// Whether `PRMONITOR_REGISTER` asked for full registration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegisterOverride {
    /// Register fully whatever the build and data root.
    Forced,
    /// Decide from the build and data root.
    Auto,
}

/// `Forced` wins; otherwise only a release build on the default root registers fully.
pub const fn registration_scope(
    scope: InstanceScope,
    build: BuildKind,
    register: RegisterOverride,
) -> RegistrationScope {
    match (register, scope, build) {
        (RegisterOverride::Forced, _, _)
        | (RegisterOverride::Auto, InstanceScope::Default, BuildKind::Release) => {
            RegistrationScope::Full
        }
        (RegisterOverride::Auto, InstanceScope::Default, BuildKind::Debug)
        | (RegisterOverride::Auto, InstanceScope::Isolated, _) => {
            RegistrationScope::ToastOnlyIfMissing
        }
    }
}

/// Whether startup may point the launch-at-login entry at this exe. An isolated root owns its
/// own entry; the shared one follows the same rule as the protocol, so a dev build never
/// takes over the installed app's entry.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "only the Windows Run value is healed at startup")
)]
pub(crate) const fn should_heal_autostart(
    scope: InstanceScope,
    registration: RegistrationScope,
) -> bool {
    match (scope, registration) {
        (InstanceScope::Isolated, _) | (InstanceScope::Default, RegistrationScope::Full) => true,
        (InstanceScope::Default, RegistrationScope::ToastOnlyIfMissing) => false,
    }
}

/// Fewest path components an isolated root may have before `--purge` deletes it.
const MIN_ISOLATED_DEPTH: usize = 3;

/// `path` in a form two spellings of one folder share: lexically normalized, then resolved
/// when it exists (without the `\\?\` prefix), lowercase on Windows.
fn comparable(path: &Path) -> PathBuf {
    let normalized = normalize_root(path);
    let resolved = std::fs::canonicalize(&normalized).map_or(normalized, strip_verbatim);
    if cfg!(windows) {
        PathBuf::from(resolved.to_string_lossy().to_lowercase())
    } else {
        resolved
    }
}

/// `\\?\C:\x` becomes `C:\x`; other paths are unchanged.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let stripped = path
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
        .filter(|rest| rest.as_bytes().get(1) == Some(&b':'))
        .map(PathBuf::from);
    stripped.unwrap_or(path)
}

/// `--purge` deletes only what is clearly PR Monitor's data folder. `user_home` is the user's
/// home directory (never the `--home` override): an isolated root may not be it or contain it.
pub(crate) fn safe_to_purge(root: &Path, scope: InstanceScope, user_home: Option<&Path>) -> bool {
    match scope {
        InstanceScope::Default => root.file_name().is_some_and(|name| name == DATA_DIR_NAME),
        InstanceScope::Isolated => {
            let ours = comparable(root);
            ours.components().count() >= MIN_ISOLATED_DEPTH
                && user_home.is_none_or(|home| !comparable(home).starts_with(&ours))
                && (root.join("config.toml").is_file() || root.join("state.json").is_file())
        }
    }
}

#[cfg(test)]
mod tests;
