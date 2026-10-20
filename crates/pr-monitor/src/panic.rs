//! Panics: one hook, installed first. A crash log next to the logs once their folder is known
//! (first five only); one dialog for a main-thread panic (startup or after the window was up);
//! then the default hook. Background panics in jobs are caught by the service.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use crate::{build_info, copy, logging::redact::redact};

/// Crash files written per run.
const MAX_REPORTS: u32 = 5;

/// Where crash reports go; unset until the data folder exists (no reports before that).
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Only the first main-thread panic shows a dialog.
static SHOWN: AtomicBool = AtomicBool::new(false);

/// What a crash report says.
pub(crate) struct CrashInfo<'a> {
    pub message: &'a str,
    pub location: Option<String>,
    pub thread: &'a str,
    pub image_base: Option<usize>,
    pub backtrace: &'a str,
}

/// Which dialog a panic shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PanicDialog {
    /// Before the first frame.
    Startup,
    /// After the window was up (otherwise the tray icon would just disappear).
    Crashed,
}

/// Plain text; `image_base` lets a PDB symbolize the addresses offline.
pub(crate) fn crash_report(info: &CrashInfo<'_>) -> String {
    let base = info
        .image_base
        .map_or_else(|| "unknown".to_owned(), |b| format!("0x{b:x}"));
    format!(
        "PR Monitor crashed.\nversion={} sha={} target={}\nthread={}\nlocation={}\nimage_base={base}\nmessage={}\n\n{}\n",
        build_info::VERSION,
        build_info::GIT_SHA,
        build_info::TARGET,
        info.thread,
        info.location.as_deref().unwrap_or("unknown"),
        info.message,
        info.backtrace,
    )
}

/// Only main-thread panics show a dialog, and only the first one (`shown` is set by the call).
pub(crate) fn panic_dialog(
    in_init: bool,
    on_main: bool,
    shown: &AtomicBool,
) -> Option<PanicDialog> {
    if !on_main || shown.swap(true, Ordering::Relaxed) {
        return None;
    }
    Some(if in_init {
        PanicDialog::Startup
    } else {
        PanicDialog::Crashed
    })
}

/// No panic dialogs from now on (console-only modes, which also run unattended in CI).
pub(crate) fn suppress_dialogs() {
    SHOWN.store(true, Ordering::Relaxed);
}

/// Crash reports go to `logs` from now on (the first call wins).
pub(crate) fn set_log_dir(logs: &Path) {
    let _ = LOG_DIR.set(logs.to_path_buf());
}

/// Installs the hook; the returned flag is true until the first frame.
pub(crate) fn install() -> Arc<AtomicBool> {
    let init_phase = Arc::new(AtomicBool::new(true));
    let count = AtomicU32::new(0);
    let in_init = Arc::clone(&init_phase);
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let message = panic.payload_as_str().unwrap_or("non-string panic payload");
        let report = LOG_DIR
            .get()
            .filter(|_| count.fetch_add(1, Ordering::Relaxed) < MAX_REPORTS)
            .and_then(|dir| {
                write_report(
                    dir,
                    &CrashInfo {
                        message,
                        location: panic.location().map(ToString::to_string),
                        thread: name,
                        image_base: prm_platform::process::image_base(),
                        backtrace: &std::backtrace::Backtrace::force_capture().to_string(),
                    },
                )
            });
        tracing::error!(target: "panic", thread = name, message, "panic");
        show_dialog(
            panic_dialog(in_init.load(Ordering::Relaxed), name == "main", &SHOWN),
            message,
            report.as_deref(),
        );
        default(panic);
    }));
    init_phase
}

fn show_dialog(dialog: Option<PanicDialog>, message: &str, report: Option<&Path>) {
    match dialog {
        Some(PanicDialog::Startup) => {
            prm_platform::console::fatal_dialog(copy::STARTUP_FAILED, message);
        }
        Some(PanicDialog::Crashed) => {
            prm_platform::console::fatal_dialog(
                copy::CRASHED_TITLE,
                &copy::crashed(message, report),
            );
        }
        None => {}
    }
}

/// Writes one redacted report; the path when it was written.
pub(crate) fn write_report(dir: &Path, info: &CrashInfo<'_>) -> Option<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("crash-{secs}-{}.log", std::process::id()));
    std::fs::write(&path, redact(&crash_report(info)).as_bytes())
        .ok()
        .map(|()| path)
}

/// Keeps the newest `keep` crash logs.
pub(crate) fn prune_crash_logs(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut crashes: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    crashes.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, path) in crashes.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests;
