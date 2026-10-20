//! Process and session facts, and the foreground-window handoff to the primary instance.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use std::time::Duration;

use windows::{
    Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::{
            LibraryLoader::GetModuleHandleW,
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetCurrentProcessId, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow},
    },
    core::PCWSTR,
};

/// The Terminal Services session of a process.
pub(crate) fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0_u32;
    // SAFETY: `session` is a live local the call writes one u32 to.
    unsafe { ProcessIdToSessionId(pid, &raw mut session) }
        .ok()
        .map(|()| session)
}

pub(crate) fn session_id() -> Option<u32> {
    // SAFETY: no preconditions.
    let pid = unsafe { GetCurrentProcessId() };
    session_of(pid)
}

/// `true` when the process exited within `timeout` (or was already gone).
pub(crate) fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    // SAFETY: no memory preconditions; the returned handle is closed below.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
        return true;
    };
    let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `handle` is open until the CloseHandle below.
    let event = unsafe { WaitForSingleObject(handle, millis) };
    // SAFETY: we own `handle` and close it exactly once.
    let _ = unsafe { CloseHandle(handle) };
    event == WAIT_OBJECT_0
}

/// This executable's load address, for symbolizing crash logs.
pub(crate) fn image_base() -> Option<usize> {
    // SAFETY: a null name returns this module's handle; nothing needs freeing.
    unsafe { GetModuleHandleW(PCWSTR::null()) }
        .ok()
        .map(|module| module.0.addr())
}

/// Lets `pid` (or any process, when unknown) take the foreground, so the primary can raise
/// its window when a secondary forwards an activation.
pub(crate) fn allow_foreground(pid: Option<u32>) {
    // SAFETY: no memory preconditions; failure only means the window may not come forward.
    let _ = unsafe { AllowSetForegroundWindow(pid.unwrap_or(ASFW_ANY)) };
}
