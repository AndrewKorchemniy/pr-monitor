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
/// but this makes it explicit). Other OSes: the socket is per-user already.
pub(crate) fn peer_in_our_session(conn: &impl StreamCommon) -> bool {
    #[cfg(windows)]
    let same = conn
        .peer_creds()
        .ok()
        .and_then(|creds| creds.pid())
        .is_some_and(|pid| {
            crate::win::process::session_of(pid) == crate::win::process::session_id()
        });
    #[cfg(not(windows))]
    let same = {
        let _ = conn;
        true
    };
    same
}

/// Before forwarding: refuse a primary in another session, and let it take the foreground.
pub(crate) fn prepare_foreground_handoff(stream: &impl StreamCommon) -> bool {
    #[cfg(windows)]
    {
        use crate::win::process::{allow_foreground, session_id, session_of};
        let pid = stream.peer_creds().ok().and_then(|creds| creds.pid());
        if pid.is_some_and(|pid| session_of(pid) != session_id()) {
            return false;
        }
        allow_foreground(pid);
    }
    #[cfg(not(windows))]
    let _ = stream;
    true
}
