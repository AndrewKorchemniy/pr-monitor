//! Messages the exe shows itself (before or after the UI exists).

pub(crate) const STARTUP_FAILED: &str = "PR Monitor couldn't start";
pub(crate) const NOT_RESPONDING: &str =
    "PR Monitor is already running but isn't responding. Close it in Task Manager and try again.";
pub(crate) const DATA_FOLDER: &str = "PR Monitor can't create its data folder";

pub(crate) fn graphics_failed(detail: &str) -> String {
    format!("Neither graphics backend could start on this computer.\n\n{detail}")
}

pub(crate) fn usage_error(error: &str, usage: &str) -> String {
    format!("{error}\n\n{usage}")
}
