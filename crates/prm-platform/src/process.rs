//! Process helpers: session ids, waiting for a predecessor to exit, crash-log image base, and
//! the handoff peer checks.

use std::time::Duration;

use interprocess::local_socket::traits::StreamCommon;

/// The Windows session id; `None` elsewhere.
pub fn session_id() -> Option<u32> {
    #[cfg(windows)]
    let id = crate::win::process::session_id();
    #[cfg(not(windows))]
    let id = None;
    id
}

/// `true` when `pid` exited within `timeout` (always `true` off Windows; the lock retry covers it).
pub fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    #[cfg(windows)]
    let exited = crate::win::process::wait_for_exit(pid, timeout);
    #[cfg(not(windows))]
    let exited = {
        let _ = (pid, timeout);
        true
    };
    exited
}

/// This executable's load address, for symbolizing crash logs.
pub fn image_base() -> Option<usize> {
    #[cfg(windows)]
    let base = crate::win::process::image_base();
    #[cfg(not(windows))]
    let base = None;
    base
}

/// Windows: the connecting process must be in our session (pipes are session-scoped by name,
/// but this makes it explicit). Linux: it must be our user, because the abstract socket
/// namespace has no permissions. macOS: the socket file sits in the per-user temp dir.
pub(crate) fn peer_in_our_session(conn: &impl StreamCommon) -> bool {
    #[cfg(windows)]
    let same = conn
        .peer_creds()
        .ok()
        .and_then(|creds| creds.pid())
        .is_some_and(|pid| {
            crate::win::process::session_of(pid) == crate::win::process::session_id()
        });
    #[cfg(target_os = "linux")]
    let same = is_our_user(conn);
    #[cfg(all(not(windows), not(target_os = "linux")))]
    let same = {
        let _ = conn;
        true
    };
    same
}

/// Before forwarding: refuse a primary in another session (Windows) or of another user
/// (Linux), and let it take the foreground.
pub(crate) fn prepare_foreground_handoff(stream: &impl StreamCommon) -> bool {
    #[cfg(windows)]
    {
        use crate::win::process::{allow_foreground, session_id, session_of};
        let pid = stream.peer_creds().ok().and_then(|creds| creds.pid());
        if pid.is_some_and(|pid| session_of(pid) != session_id()) {
            return false;
        }
        allow_foreground(pid);
        true
    }
    #[cfg(target_os = "linux")]
    {
        is_our_user(stream)
    }
    #[cfg(all(not(windows), not(target_os = "linux")))]
    {
        let _ = stream;
        true
    }
}

/// Whether the peer runs as our effective user; `false` when either is unknown (fail closed:
/// anyone can connect to, or squat, an abstract socket name).
#[cfg(target_os = "linux")]
fn is_our_user(conn: &impl StreamCommon) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    // why: `/proc/self` is owned by this process's effective uid; std has no safe geteuid.
    let own = std::fs::metadata("/proc/self").map(|meta| meta.uid()).ok();
    let peer = conn.peer_creds().ok().and_then(|creds| creds.euid());
    own.is_some() && own == peer
}
