//! Panics: a crash log next to the logs (first five only), a dialog while starting up, then the
//! default hook. Background panics in jobs are caught by the service; the tray keeps running.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use crate::{build_info, logging::redact::redact};

/// Crash files written per run.
const MAX_REPORTS: u32 = 5;

/// What a crash report says.
pub(crate) struct CrashInfo<'a> {
    pub message: &'a str,
    pub location: Option<String>,
    pub thread: &'a str,
    pub image_base: Option<usize>,
    pub backtrace: &'a str,
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

/// Installs the hook; the returned flag is true until the first frame (dialogs only then).
pub(crate) fn install(logs: &Path) -> Arc<AtomicBool> {
    let init_phase = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU32::new(0));
    let dir = logs.to_path_buf();
    let in_init = Arc::clone(&init_phase);
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let message = panic.payload_as_str().unwrap_or("non-string panic payload");
        if count.fetch_add(1, Ordering::Relaxed) < MAX_REPORTS {
            write_report(
                &dir,
                &CrashInfo {
                    message,
                    location: panic.location().map(ToString::to_string),
                    thread: name,
                    image_base: prm_platform::process::image_base(),
                    backtrace: &std::backtrace::Backtrace::force_capture().to_string(),
                },
            );
        }
        tracing::error!(target: "panic", thread = name, message, "panic");
        if in_init.load(Ordering::Relaxed) && name == "main" {
            prm_platform::console::fatal_dialog("PR Monitor couldn't start", message);
        }
        default(panic);
    }));
    init_phase
}

fn write_report(dir: &Path, info: &CrashInfo<'_>) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("crash-{secs}-{}.log", std::process::id()));
    let _ = std::fs::write(path, redact(&crash_report(info)).as_bytes());
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
