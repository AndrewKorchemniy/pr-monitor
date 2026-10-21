//! The single home for identity strings and URL constants. Nothing else in the workspace
//! spells the app name, AUMID, URI scheme, registry value names or environment variables.

use serde::{Deserialize, Serialize};
use url::Url;

/// A macro, not a const, because `concat!` (used for doc links below) only accepts literals.
macro_rules! repo_url {
    () => {
        "https://github.com/AndrewKorchemniy/pr-monitor"
    };
}

pub const APP_DISPLAY_NAME: &str = "PR Monitor";
/// Stable application id (`with_app_id`, Wayland app id, log file prefix).
pub const APP_ID: &str = "pr-monitor";
pub const APP_EXE_NAME: &str = "pr-monitor.exe";
/// Windows AppUserModelID used for toasts and taskbar grouping. Never includes the version.
pub const AUMID: &str = "AndrewKorchemniy.PRMonitor";
pub const URI_SCHEME: &str = "prmonitor";
/// Folder under `%LOCALAPPDATA%`.
pub const DATA_DIR_NAME: &str = "PRMonitor";
/// keyring "service" name.
pub const SECRET_SERVICE: &str = "PRMonitor";
/// `HKCU\...\Run` value name.
pub const RUN_VALUE_NAME: &str = "PRMonitor";
/// Credential Manager target prefix (`PRMonitor/<key>`).
pub const CREDMAN_TARGET_PREFIX: &str = "PRMonitor/";
/// Default value of the `prmonitor` protocol key.
pub const PROTOCOL_DESCRIPTION: &str = "URL:PR Monitor";
pub const ENV_HOME: &str = "PRMONITOR_HOME";
pub const ENV_LOG: &str = "PRMONITOR_LOG";
/// `dpapi` opts into the DPAPI file store instead of Credential Manager.
pub const ENV_SECRET_STORE: &str = "PRMONITOR_SECRET_STORE";
/// `1` forces shell registration (AUMID, protocol) even in debug or isolated-home runs.
pub const ENV_REGISTER: &str = "PRMONITOR_REGISTER";
pub const REPO_URL: &str = repo_url!();
/// Product token in the HTTP User-Agent (no spaces allowed).
pub const USER_AGENT_PRODUCT: &str = "PRMonitor";

pub fn user_agent(version: &str) -> String {
    format!("{USER_AGENT_PRODUCT}/{version} (+{REPO_URL})")
}

/// Hosts the app may open in the browser. https only; an entry starting with `.` is a suffix match.
pub const OPENABLE_HOSTS: &[&str] = &[
    "dev.azure.com",
    ".visualstudio.com",
    "login.microsoftonline.com",
    "learn.microsoft.com",
    "aka.ms",
    "github.com",
];

pub fn is_openable_url(u: &Url) -> bool {
    if u.scheme() != "https" {
        return false;
    }
    let Some(host) = u.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    OPENABLE_HOSTS.iter().any(|allowed| {
        if allowed.starts_with('.') {
            host.len() > allowed.len() && host.ends_with(allowed)
        } else {
            host == *allowed
        }
    })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocLink {
    InstallAzureCli,
    CreatePat,
    EntraAppRegistration,
    Troubleshooting,
    /// A new GitHub issue (bug report or feature request).
    ReportProblem,
}

impl DocLink {
    pub const fn url(self) -> &'static str {
        match self {
            Self::InstallAzureCli => "https://learn.microsoft.com/cli/azure/install-azure-cli",
            Self::CreatePat => {
                "https://learn.microsoft.com/azure/devops/organizations/accounts/use-personal-access-tokens-to-authenticate"
            }
            Self::EntraAppRegistration => {
                "https://learn.microsoft.com/azure/devops/integrate/get-started/authentication/entra-oauth"
            }
            Self::Troubleshooting => concat!(repo_url!(), "#troubleshooting"),
            Self::ReportProblem => concat!(repo_url!(), "/issues/new/choose"),
        }
    }
}

#[cfg(test)]
mod tests;
