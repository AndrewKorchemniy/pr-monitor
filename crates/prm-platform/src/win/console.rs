//! `--debug` console output for a GUI-subsystem exe, and the last-resort error dialog.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use std::io::Write;

use windows::{
    Win32::{
        Foundation::ERROR_ACCESS_DENIED,
        System::Console::{ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole, GetConsoleWindow},
        UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW},
    },
    core::HSTRING,
};

/// Attaches to the launching terminal; allocates a new console when there is none and
/// `alloc_if_none` is set. `true` when a console is available, including one the process
/// already has (a console-subsystem debug build, where `AttachConsole` fails with
/// `ERROR_ACCESS_DENIED`).
pub(crate) fn attach_parent_console(alloc_if_none: bool) -> bool {
    // SAFETY: no memory preconditions.
    match unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } {
        Ok(()) => return true,
        Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => return true,
        Err(_) => {}
    }
    // SAFETY: no memory preconditions; returns null when the process has no console.
    if !unsafe { GetConsoleWindow() }.is_invalid() {
        return true;
    }
    // SAFETY: no memory preconditions.
    alloc_if_none && unsafe { AllocConsole() }.is_ok()
}

/// Standard output, or the console device when a GUI exe has no stdout handle.
pub(crate) fn console_out() -> Box<dyn Write + Send> {
    use std::os::windows::io::AsRawHandle;
    if std::io::stdout().as_raw_handle().is_null() {
        let conout = std::fs::OpenOptions::new().write(true).open("CONOUT$");
        if let Ok(file) = conout {
            return Box::new(file);
        }
    }
    Box::new(std::io::stdout())
}

pub(crate) fn fatal_dialog(title: &str, body: &str) {
    let (body, title) = (HSTRING::from(body), HSTRING::from(title));
    // SAFETY: both strings are NUL-terminated and outlive the (modal) call.
    let _ = unsafe {
        MessageBoxW(
            None,
            &body,
            &title,
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
        )
    };
}
