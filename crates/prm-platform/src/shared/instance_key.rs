//! Names for the single-instance mutex, the activation pipe/socket and the lock file. A second
//! data root (`--home`) gets its own names so two independent instances can run. Spellings of
//! one folder (trailing separator, `/` vs `\`, `.` and `..`, case on Windows) are one root.

use std::path::{Component, Path, PathBuf};

use prm_domain::{app::AUMID, hash::fnv64};

/// Which single-instance names a launch uses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstanceScope {
    /// The normal per-user instance.
    Default,
    /// A custom data root, or another user owns the default mutex in this session.
    Isolated,
}

/// Every OS name derived from one data root. Each field is meaningful on some OSes only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceKey {
    /// Windows named mutex (session-local).
    pub mutex_name: String,
    /// Pipe or socket name.
    pub pipe_name: String,
    /// The lock file every primary holds (both OSes; the only guard on Unix).
    pub lock_file: PathBuf,
    /// 8 hex digits identifying the data root.
    pub suffix: String,
}

/// `path` rebuilt from its components: no trailing separator, no `.`, `..` folded where it
/// can be, and the platform separator throughout. Purely lexical (the folder may not exist).
pub(crate) fn normalize_root(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => fold_parent(&mut parts, component),
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                parts.push(component);
            }
        }
    }
    parts.iter().collect()
}

/// `..` removes the folder before it; above the root it is dropped, in a relative path kept.
fn fold_parent<'a>(parts: &mut Vec<Component<'a>>, parent: Component<'a>) {
    match parts.last() {
        Some(Component::Normal(_)) => {
            parts.pop();
        }
        Some(Component::Prefix(_) | Component::RootDir) => {}
        Some(Component::CurDir | Component::ParentDir) | None => parts.push(parent),
    }
}

/// The text that identifies a data root: normalized, and lowercase on Windows, where paths are
/// case-insensitive.
fn root_identity(root: &Path) -> String {
    let text = normalize_root(root).to_string_lossy().into_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Whether two spellings name the same data root (lexically; symlinks are not resolved).
pub fn is_same_root(a: &Path, b: &Path) -> bool {
    root_identity(a) == root_identity(b)
}

/// The data root's short identity, the same for every spelling of it.
fn root_suffix(root: &Path) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 32 bits are enough to tell data roots apart"
    )]
    let short = fnv64(root_identity(root).as_bytes()) as u32;
    format!("{short:08x}")
}

/// `session_id` is the Windows session; `None` on Unix, where the socket name carries no session.
pub(crate) fn derive(root: &Path, scope: InstanceScope, session_id: Option<u32>) -> InstanceKey {
    let suffix = root_suffix(root);
    let (mutex_name, pipe_name) = match (scope, session_id) {
        (InstanceScope::Default, Some(session)) => {
            (format!("Local\\{AUMID}"), format!("{AUMID}.{session}"))
        }
        (InstanceScope::Isolated, Some(session)) => (
            format!("Local\\{AUMID}.{suffix}"),
            format!("{AUMID}.{session}.{suffix}"),
        ),
        (InstanceScope::Default | InstanceScope::Isolated, None) => (
            format!("Local\\{AUMID}.{suffix}"),
            format!("{AUMID}.{suffix}.sock"),
        ),
    };
    InstanceKey {
        mutex_name,
        pipe_name,
        lock_file: root.join("instance.lock"),
        suffix,
    }
}

#[cfg(test)]
mod tests;
