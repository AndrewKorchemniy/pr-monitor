//! What was built: version, commit and target (from `build.rs`).

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const GIT_SHA: &str = env!("PRM_BUILD_SHA");
pub(crate) const TARGET: &str = env!("PRM_TARGET");

pub(crate) const BUILD: prm_ui::BuildInfo = prm_ui::BuildInfo {
    version: VERSION,
    git_sha: GIT_SHA,
    target: TARGET,
};

/// "PR Monitor 0.1.0 (0123456789ab, x86_64-pc-windows-msvc)".
pub(crate) fn version_line() -> String {
    format!(
        "{} {VERSION} ({GIT_SHA}, {TARGET})",
        prm_domain::app::APP_DISPLAY_NAME
    )
}
