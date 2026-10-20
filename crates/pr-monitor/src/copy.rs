//! Messages the exe shows itself (before or after the UI exists).

use std::path::Path;

pub(crate) const STARTUP_FAILED: &str = "PR Monitor couldn't start";
pub(crate) const NOT_RESPONDING: &str =
    "PR Monitor is already running but isn't responding. Close it in Task Manager and try again.";
pub(crate) const DATA_FOLDER: &str = "PR Monitor can't create its data folder";
pub(crate) const CRASHED_TITLE: &str = "PR Monitor stopped unexpectedly";

pub(crate) fn graphics_failed(detail: &str) -> String {
    format!("Neither graphics backend could start on this computer.\n\n{detail}")
}

pub(crate) fn usage_error(error: &str, usage: &str) -> String {
    format!("{error}\n\n{usage}")
}

/// A panic on the UI thread after the window was up; `report` is the crash log, when written.
pub(crate) fn crashed(message: &str, report: Option<&Path>) -> String {
    let report = report.map_or_else(String::new, |p| {
        format!("\n\nA crash report was saved to:\n{}", p.display())
    });
    format!("PR Monitor hit an error and has to close.\n\n{message}{report}")
}
