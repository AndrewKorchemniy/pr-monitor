//! Is this a remote desktop (RDP, RemoteFX)? Picks the renderer.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};
use windows_registry::LOCAL_MACHINE;

use crate::shared::session_kind::{SessionInfo, classify_session};

pub(crate) fn session_info() -> SessionInfo {
    let id = super::process::session_id();
    // SAFETY: no preconditions.
    let sm_remote = unsafe { GetSystemMetrics(SM_REMOTESESSION) } != 0;
    let glass = LOCAL_MACHINE
        .open(r"SYSTEM\CurrentControlSet\Control\Terminal Server")
        .and_then(|key| key.get_u32("GlassSessionId"))
        .ok();
    // Microsoft-documented RemoteFX check: the console ("glass") session is not ours.
    let glass_mismatch = matches!((glass, id), (Some(g), Some(s)) if g != s);
    let name = std::env::var("SESSIONNAME").ok();
    let remote_agent_host = LOCAL_MACHINE
        .open(r"SOFTWARE\Vendor\RemoteAgent")
        .is_ok();
    SessionInfo {
        kind: classify_session(sm_remote, glass_mismatch, name.as_deref()),
        id,
        remote_agent_host,
    }
}
