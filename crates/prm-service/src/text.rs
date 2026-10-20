//! User-facing strings for banners, notices and `UiError`s raised by the service. Sentence case,
//! active voice; errors say what happened and what to do.

use std::sync::Arc;

use prm_domain::scope::ScopeIssue;

pub(crate) const INTERNAL_ERROR: &str = "A background task failed. See the log for details.";
pub(crate) const FORBIDDEN: &str = "Access denied";
pub(crate) const NOT_FOUND: &str = "Not found";
pub(crate) const RATE_LIMITED: &str =
    "Azure DevOps is throttling requests, so PR Monitor is slowing down";
pub(crate) const UNREACHABLE: &str = "Can't reach Azure DevOps";
pub(crate) const TIMED_OUT: &str = "Azure DevOps didn't respond in time";
pub(crate) const UNEXPECTED_RESPONSE: &str = "Unexpected response from Azure DevOps";
pub(crate) const HTTP_ERROR: &str = "Azure DevOps returned an error";
pub(crate) const PROXY_AUTH: &str =
    "Your proxy requires sign-in that PR Monitor can't do yet (NTLM or Kerberos)";
pub(crate) const CANCELLED: &str = "Cancelled";
pub(crate) const REFRESH_TIMED_OUT: &str = "Refreshing took too long and was stopped";
pub(crate) const SETTINGS_NOT_SAVED: &str = "Couldn't save settings";
pub(crate) const STATE_NOT_SAVED: &str =
    "Couldn't save PR Monitor's data. Changes may be lost on exit.";
pub(crate) const CATALOG_FAILED: &str = "Couldn't load projects and repositories";
pub(crate) const SEARCH_FAILED: &str = "Couldn't search the directory; showing known people only";
pub(crate) const PERSON_UNRESOLVED: &str = "Couldn't look up this person";
pub(crate) const LINK_BLOCKED: &str = "That link doesn't go to Azure DevOps, so it wasn't opened";
pub(crate) const OPEN_FAILED: &str = "Couldn't open the browser";
pub(crate) const NOTIFICATION_FAILED: &str = "Couldn't show a notification";
pub(crate) const AUTOSTART_FAILED: &str = "Couldn't change launch at login";
pub(crate) const NETWORK_RESTART: &str = "Network settings apply after you restart PR Monitor";
pub(crate) const OFFLINE: &str = "Can't reach Azure DevOps. Showing the last data.";
pub(crate) const CONFIG_ERRORS: &str = "Some settings couldn't be used";
pub(crate) const NOTIFY_BLOCKED_BY_USER: &str =
    "Notifications for PR Monitor are off in Windows settings";
pub(crate) const NOTIFY_BLOCKED_BY_POLICY: &str = "Your organization turned off notifications";
pub(crate) const AUTOSTART_DISABLED: &str = "Launch at login is turned off in Task Manager";

pub(crate) fn sign_in_needed(org: &str) -> String {
    format!("Sign in to {org}")
}

pub(crate) fn pat_expiry(org: &str, days: i64) -> String {
    match days {
        ..=0 => format!("The token for {org} has expired"),
        1 => format!("The token for {org} expires tomorrow"),
        _ => format!("The token for {org} expires in {days} days"),
    }
}

pub(crate) fn degraded(failed_queries: usize) -> String {
    match failed_queries {
        1 => "1 query failed; some PRs may be out of date".to_owned(),
        n => format!("{n} queries failed; some PRs may be out of date"),
    }
}

pub(crate) fn scope_issue(issue: &ScopeIssue) -> Arc<str> {
    let text = match issue {
        ScopeIssue::ProjectNotFound(name) => format!("Project {name} wasn't found"),
        ScopeIssue::RepoNotFound { project, repo } => {
            format!("Repository {repo} wasn't found in {project}")
        }
        ScopeIssue::RepoDisabled(name) => format!("Repository {name} is disabled"),
    };
    Arc::from(text)
}
