//! Names for the single-instance mutex, the activation pipe/socket and the lock file. A second
//! data root (`--home`) gets its own names so two independent instances can run.

use std::path::{Path, PathBuf};

use prm_domain::{app::AUMID, hash::fnv64};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstanceScope {
    /// The normal per-user instance.
    Default,
    /// A custom data root, or another user owns the default mutex in this session.
    Isolated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceKey {
    /// Windows named mutex (session-local).
    pub mutex_name: String,
    /// Pipe or socket name.
    pub pipe_name: String,
    /// Unix lock file.
    pub lock_file: PathBuf,
    /// 8 hex digits identifying the data root.
    pub suffix: String,
}

/// The data root's identity: case-insensitive on Windows, where paths are.
fn root_suffix(root: &Path) -> String {
    let text = root.to_string_lossy();
    let normalized = if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.into_owned()
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 32 bits are enough to tell data roots apart"
    )]
    let short = fnv64(normalized.as_bytes()) as u32;
    format!("{short:08x}")
}

/// `session_id` is the Windows session; `None` on Unix, where the socket name carries no session.
pub fn derive(root: &Path, scope: InstanceScope, session_id: Option<u32>) -> InstanceKey {
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
