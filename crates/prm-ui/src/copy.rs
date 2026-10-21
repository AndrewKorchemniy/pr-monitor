//! Every user-facing UI string. Sentence case, plain verbs; errors say what happened and what to
//! do. Toast text lives in `prm_domain::notify::text`, banner and error titles in `prm_service`.

use prm_domain::{
    PrStatus,
    auth::{AuthMethod, SettingsSection, UserAction},
    classify::VoteTally,
    diff::ChangeKind,
    model::VoteKind,
    tombstone::CloseOutcome,
};

use crate::format::plural;

pub(crate) const FILTER_HINT: &str = "Filter   @author  repo:name  !123  is:draft  is:unread";
pub(crate) const NOT_UPDATED: &str = "Not updated yet";
pub(crate) const UPDATED_NOW: &str = "Updated just now";
pub(crate) const SLOWED: &str = "Slowed down";
pub(crate) const THROTTLED: &str =
    "Azure DevOps is throttling requests, so PR Monitor refreshes less often until it recovers.";
pub(crate) const SIGNING_IN: &str = "Signing in…";
pub(crate) const LOADING_PROJECTS: &str = "Loading projects…";
pub(crate) const PROJECTS_FAILED: &str = "Couldn't load projects";
pub(crate) const CHANGED_TOOLTIP: &str = "Changed since you last looked";
pub(crate) const DRAFT: &str = "Draft";
pub(crate) const GROUP: &str = "Group";
pub(crate) const CONFLICTS: &str = "Merge conflicts";
pub(crate) const REVIEWS: &str = "Reviews";
pub(crate) const NO_OPEN_THREADS: &str = "No open threads.";
pub(crate) const OPEN_THREADS: &str = "Open threads";
pub(crate) const NO_REVIEWS: &str = "No reviews yet.";
pub(crate) const GROUP_TAG: &str = "group";
/// After a team's name on its pill.
pub(crate) const GROUP_SUFFIX: &str = " · group";
pub(crate) const REQUIRED_TAG: &str = "required";
pub(crate) const OPEN_IN_BROWSER: &str = "Open in browser";
pub(crate) const COPY_LINK: &str = "Copy link";
pub(crate) const COPY_MARKDOWN: &str = "Copy Markdown link";
pub(crate) const COPY_TITLE: &str = "Copy title";
pub(crate) const PEEK: &str = "Peek";
pub(crate) const MARK_SEEN: &str = "Mark as seen";
pub(crate) const CLOSE: &str = "Close";

pub(crate) const COLUMN_STATUS: &str = "Status";
pub(crate) const COLUMN_NUMBER: &str = "PR";
pub(crate) const COLUMN_FLAGS: &str = "Flags";
pub(crate) const COLUMN_TITLE: &str = "Title";
pub(crate) const COLUMN_REPO: &str = "Repository";
pub(crate) const COLUMN_AUTHOR: &str = "Author";
pub(crate) const COLUMN_COMMENTS: &str = "Comments";
pub(crate) const COLUMN_AGE: &str = "Age";
pub(crate) const COLUMN_LOCKED: &str = "Always shown";
pub(crate) const COLUMN_SQUEEZED: &str = "Hidden while the table is this narrow";
pub(crate) const AUTO_COMPLETE: &str = "Auto-complete is set";
pub(crate) const COLUMNS: &str = "Columns";
pub(crate) const COLUMNS_HELP: &str = "Status and Title always show. Changes apply right away, and you can also right-click a column header.";
pub(crate) const SERVICE_STOPPED: &str =
    "PR Monitor's background service stopped. Restart the app.";

pub(crate) const EMPTY_LOADING: &str = "Loading pull requests";
pub(crate) const EMPTY_SIGNED_OUT: &str = "You're signed out.";
pub(crate) const EMPTY_SIGNED_OUT_BODY: &str =
    "Sign in to an organization to see its pull requests.";
pub(crate) const EMPTY_SIGN_IN: &str = "Sign in";
pub(crate) const EMPTY_NO_PRS: &str = "No active pull requests.";
pub(crate) const EMPTY_EDIT_PEOPLE: &str = "Edit tracked people";
pub(crate) const CLEAR_FILTER: &str = "Clear filter";
pub(crate) const SHOW_ALL: &str = "Show all";
pub(crate) const NOTHING_CHANGED: &str = "Nothing changed since you last looked.";

pub(crate) const APP_NAME: &str = "PR Monitor";
pub(crate) const ORG_URL_LABEL: &str = "Organization URL";
pub(crate) const ORG_URL_HINT: &str = "https://dev.azure.com/your-org";
pub(crate) const CONNECT: &str = "Connect";
pub(crate) const BAD_ORG_URL: &str =
    "That isn't an Azure DevOps organization URL. Try https://dev.azure.com/your-org.";
pub(crate) const STEP_REPOS: &str = "Choose repositories";
pub(crate) const REPOSITORIES: &str = "Repositories";
pub(crate) const PEOPLE: &str = "People";
pub(crate) const CONTINUE: &str = "Continue";
pub(crate) const PREVIOUS: &str = "Back";
pub(crate) const DONE: &str = "Done";
pub(crate) const CHOOSE: &str = "Choose";
pub(crate) const EDIT: &str = "Edit";
pub(crate) const NOBODY_YET: &str = "Nobody yet";
pub(crate) const TRACK_FROM: &str = "Track pull requests from";
pub(crate) const TRACK_FROM_HELP: &str =
    "Leave this empty to track everyone in the repositories you chose.";
pub(crate) const EVERYONE: &str = "Everyone";
pub(crate) const EVERYONE_BUT_ME: &str = "Everyone except you";
pub(crate) const COUNT_REVIEWS_FROM: &str = "Count reviews from";
pub(crate) const REVIEWERS_EMPTY_HELP: &str =
    "Leave this empty to count any vote except the author's.";
pub(crate) const START_MONITORING: &str = "Start monitoring";

pub(crate) const SIGN_IN_MICROSOFT: &str = "Sign in with Microsoft";
pub(crate) const SIGN_IN_BROWSER: &str = "Sign in with the browser instead";
pub(crate) const USE_PAT: &str = "Use a personal access token";
pub(crate) const SIGN_IN_ENTRA: &str = "Sign in with your organization's app";
pub(crate) const SIGN_OUT: &str = "Sign out";
pub(crate) const CHANGE_SIGN_IN: &str = "Change sign-in";
pub(crate) const SAVE_ORG_FIRST: &str = "Save this organization to sign in.";
pub(crate) const CHECKING_SIGN_IN: &str = "Checking sign-in…";
pub(crate) const WAITING_SIGN_IN: &str = "Waiting for you to finish signing in in your browser…";
pub(crate) const CANCEL: &str = "Cancel";
pub(crate) const SIGNED_OUT: &str = "Signed out";
pub(crate) const PAT_LABEL: &str = "Token";
pub(crate) const PAT_HELP: &str =
    "Needs Code (Read), Project and Team (Read), Graph (Read) and Identity (Read).";
pub(crate) const HOW_TO_CREATE_TOKEN: &str = "How to create a token";
pub(crate) const SAVE_TOKEN: &str = "Save token";
pub(crate) const ENTRA_CLIENT_ID: &str = "Client ID";
pub(crate) const ENTRA_TENANT: &str = "App tenant";
pub(crate) const ENTRA_HELP: &str = "Ask your admin for a public client app registered with an http://localhost redirect. PR Monitor never fills this in for you.";
pub(crate) const ADMIN_TITLE: &str = "Ask your admin for access";
pub(crate) const COPY_MESSAGE: &str = "Copy message";

pub(crate) const UNSAVED: &str = "You have unsaved changes.";
pub(crate) const DISCARD: &str = "Discard";
pub(crate) const SAVE_CHANGES: &str = "Save changes";
pub(crate) const SAVE_FAILED: &str = "the settings file couldn't be written";
pub(crate) const DISCARD_TITLE: &str = "Discard changes?";
pub(crate) const DISCARD_BODY: &str = "Your edits to settings will be lost.";
pub(crate) const KEEP_EDITING: &str = "Keep editing";
pub(crate) const START_MINIMIZED: &str = "Start minimized to the tray";
pub(crate) const CLOSE_TO_TRAY: &str = "Keep running in the tray when I close the window";
pub(crate) const MINIMIZE_TO_TRAY: &str = "Hide to the tray when minimized";
pub(crate) const LAUNCH_AT_LOGIN: &str = "Start PR Monitor when I sign in to Windows";
pub(crate) const REFRESH_HELP: &str = "5–10 minutes is plenty for large organizations.";
pub(crate) const ORG_COLUMN: &str = "Organization column";
pub(crate) const ORG_COLUMN_HELP: &str =
    "Automatic shows it when you watch more than one organization.";
pub(crate) const PROXY: &str = "Proxy";
pub(crate) const PROXY_HINT: &str = "system, none, pac or http://proxy:8080";
pub(crate) const RELATIVE_TIMES: &str = "Show relative times (2h) instead of dates";
pub(crate) const ADD_ORG: &str = "Add organization";
pub(crate) const NO_ORGS: &str =
    "No organizations yet. Add one to start watching its pull requests.";
pub(crate) const ORGANIZATION: &str = "Organization";
pub(crate) const SECTION_REFRESH: &str = "Refresh";
pub(crate) const SECTION_WINDOW: &str = "Window and tray";
pub(crate) const SECTION_TABLE: &str = "Table";
pub(crate) const SECTION_NETWORK: &str = "Network";
pub(crate) const SECTION_DELIVERY: &str = "Delivery";
pub(crate) const SECTION_EVENTS: &str = "Events";
pub(crate) const SECTION_TRY: &str = "Test and pause";
pub(crate) const SECTION_LAYOUT: &str = "Layout";
pub(crate) const SECTION_PERFORMANCE: &str = "Performance";
pub(crate) const SECTION_CUSTOM_THEMES: &str = "Custom themes";
pub(crate) const SECTION_PRIVACY: &str = "Privacy";
pub(crate) const SETTINGS: &str = "Settings";
pub(crate) const LICENSES_SHORT: &str = "Licenses";
pub(crate) const DOUBLE_CLICK_TO_TYPE: &str = "Double-click to type a value";
pub(crate) const REFRESH_INTERVAL: &str = "Refresh interval";
pub(crate) const PER_REFRESH: &str = "Notifications per refresh";
pub(crate) const TEXT_SIZE: &str = "Text size";
pub(crate) const UNIT_MINUTES: &str = " min";
pub(crate) const UNIT_PERCENT: &str = "%";
pub(crate) const NAME: &str = "Name";
pub(crate) const WATCH_ORG: &str = "Watch this organization";
pub(crate) const MUTE_ORG: &str = "Mute notifications from this organization";
pub(crate) const SIGN_IN_METHOD: &str = "Sign-in method";
pub(crate) const CONNECTION: &str = "Connection";
pub(crate) const TENANT_HELP: &str = "Leave empty to find it from the organization.";
pub(crate) const ENTRA_TENANT_HELP: &str = "Leave empty to use the organization's tenant.";
pub(crate) const ADVANCED: &str = "Advanced";
pub(crate) const TENANT_ID: &str = "Tenant ID";
pub(crate) const TOKEN_EXPIRES: &str = "Token expires on";
pub(crate) const REMOVE_ORG: &str = "Remove organization";
pub(crate) const REMOVE_ORG_BODY: &str = "PR Monitor stops watching it and forgets its cached pull requests. Saved sign-ins stay until you sign out.";
pub(crate) const REMOVE: &str = "Remove";
pub(crate) const WATCH_ALL_PROJECTS: &str = "Watch all projects";
pub(crate) const REFRESH_LIST: &str = "Refresh list";
pub(crate) const EXPAND: &str = "Expand";
pub(crate) const COLLAPSE: &str = "Collapse";
pub(crate) const INCLUDE_DRAFTS: &str = "Include draft pull requests";
pub(crate) const ORG_WIDE_QUERY: &str = "Use one request for the whole organization (experimental)";
pub(crate) const INCLUDE_ME: &str = "Include my own pull requests";
pub(crate) const REVIEWERS_COUNT: &str = "Reviewers whose votes count";
pub(crate) const REVIEWERS_HELP: &str =
    "Votes from anyone else are ignored. Add a team to count its combined vote.";
pub(crate) const EMPTY_LIST_POLICY: &str = "If this list is empty";
pub(crate) const ANY_NON_AUTHOR: &str = "Count any vote except the author's";
pub(crate) const NO_VOTES: &str = "Count no votes";
pub(crate) const IGNORE_FROM: &str = "Ignore comments from";
pub(crate) const IGNORE_HELP: &str = "Bots and pipelines.";
pub(crate) const ADD_PEOPLE: &str = "Add people";
pub(crate) const SEARCH_MIN: &str = "Type 3 or more letters to search the directory.";
pub(crate) const SEARCHING: &str = "Searching…";
pub(crate) const SIGN_IN_TO_SEARCH: &str = "Sign in to search the directory.";
pub(crate) const ADDING: &str = "Adding…";
pub(crate) const SHOW_NOTIFICATIONS: &str = "Show Windows notifications";
pub(crate) const WHILE_IN_FRONT: &str = "While PR Monitor is in front";
pub(crate) const DONT_NOTIFY: &str = "Don't notify";
pub(crate) const NOTIFY_ANYWAY: &str = "Notify anyway";
pub(crate) const PLAY_SOUND: &str = "Play a sound";
pub(crate) const SEND_TEST: &str = "Send a test notification";
pub(crate) const TEST_SENT: &str = "Test notification sent.";
pub(crate) const PAUSE_HOUR: &str = "Pause for 1 hour";
pub(crate) const RESUME: &str = "Resume";
pub(crate) const THEME: &str = "Theme";
pub(crate) const ANIMATIONS: &str = "Animations";
pub(crate) const ANIMATIONS_HELP: &str = "Automatic turns animations off over Remote Desktop, with software rendering, and when Windows animations are off.";
pub(crate) const LOW_POWER: &str = "Low-power mode";
pub(crate) const RENDERER: &str = "Renderer";
pub(crate) const RESTART_HELP: &str = "Takes effect after restart.";
pub(crate) const RELOAD_THEMES: &str = "Reload themes";
pub(crate) const COPY_THEME_TOML: &str = "Copy theme as TOML";
pub(crate) const COPY_THEMES_FOLDER: &str = "Copy themes folder path";
pub(crate) const PRIVACY: &str = "PR Monitor talks only to Azure DevOps and Microsoft sign-in. Settings and cached results stay on this computer in %LOCALAPPDATA%\\PRMonitor. Tokens are kept in Windows Credential Manager. Nothing is sent anywhere else.";
pub(crate) const LICENSES: &str = "Third-party licenses";
pub(crate) const SOURCE_CODE: &str = "Source code";
pub(crate) const REPORT_PROBLEM: &str = "Report a problem";
pub(crate) const PALETTE_HINT: &str = "Type a command, or ! and a number to jump";
pub(crate) const PALETTE_EMPTY: &str = "No matching commands.";
pub(crate) const PALETTE_FOOTER: &str = "Enter to run · Esc to close";
pub(crate) const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";
pub(crate) const TRAY_OPEN: &str = "Open PR Monitor";
pub(crate) const TRAY_REFRESH: &str = "Refresh now";
pub(crate) const TRAY_PAUSE: &str = "Pause notifications for 1 hour";
pub(crate) const TRAY_RESUME: &str = "Resume notifications";
pub(crate) const TRAY_SIGN_IN: &str = "Sign in…";
pub(crate) const TRAY_SETTINGS: &str = "Settings";
pub(crate) const TRAY_QUIT: &str = "Quit PR Monitor";

pub(crate) fn status_tooltip(status: PrStatus, tally: VoteTally, active_threads: u32) -> String {
    match status {
        PrStatus::NeedsReview => "No one on your reviewer list has voted yet.".to_owned(),
        PrStatus::ActiveComments => format!(
            "{} waiting on the author.",
            plural(active_threads, "open thread", "open threads")
        ),
        PrStatus::Reviewed => reviewed_tooltip(tally),
    }
}

fn reviewed_tooltip(tally: VoteTally) -> String {
    let parts: Vec<String> = [
        (tally.approved, "approved"),
        (tally.suggestions, "with suggestions"),
        (tally.waiting, "waiting for author"),
        (tally.rejected, "rejected"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, label)| format!("{n} {label}"))
    .collect();
    format!("Reviewed: {}", parts.join(" · "))
}

pub(crate) const fn vote_word(vote: VoteKind) -> &'static str {
    match vote {
        VoteKind::Approved => "Approved",
        VoteKind::ApprovedWithSuggestions => "Approved with suggestions",
        VoteKind::WaitingForAuthor => "Waiting for author",
        VoteKind::Rejected => "Rejected",
        VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => "No vote",
    }
}

pub(crate) fn status_count(count: u32, status: PrStatus) -> String {
    format!("{count} {}", status.label())
}

pub(crate) fn changed_count(count: u32) -> String {
    format!("{count} Changed")
}

pub(crate) fn identity(org: &str, method: Option<AuthMethod>) -> String {
    match method {
        Some(method) => format!("{org} · {}", method.label()),
        None => org.to_owned(),
    }
}

pub(crate) fn organizations(n: usize) -> String {
    format!("{n} organizations")
}

pub(crate) fn updated_ago(age: &str) -> String {
    format!("Updated {age} ago")
}

pub(crate) fn next_in_minutes(minutes: u64) -> String {
    format!("Next in {minutes}m")
}

pub(crate) fn next_in_seconds(seconds: u64) -> String {
    format!("Next in 0:{seconds:02}")
}

pub(crate) fn paused_until(hh_mm: &str) -> String {
    format!("Notifications paused until {hh_mm}")
}

pub(crate) fn listing_progress(done: u32, total: u32) -> String {
    format!("Listing pull requests {done}/{total}")
}

pub(crate) fn reading_progress(done: u32, total: u32) -> String {
    format!("Reading comments {done}/{total}")
}

pub(crate) fn repos(n: u32) -> String {
    format!("{n} repos")
}

pub(crate) fn repos_failed(ok: u32, total: u32, failed: u32) -> String {
    format!("{ok}/{total} repos · {failed} failed")
}

pub(crate) fn stale_row(hh_mm: &str) -> String {
    format!("Couldn't refresh this pull request. Showing data from {hh_mm}.")
}

/// "feature/x into main".
pub(crate) fn branches(source: &str, target: &str) -> String {
    format!("{source} into {target}")
}

pub(crate) fn open_threads(active: u32, total: u32) -> String {
    format!("Open threads ({active} of {total})")
}

pub(crate) fn opened_activity(opened: &str, activity: &str) -> String {
    format!("Opened {opened} ago · Last activity {activity} ago")
}

pub(crate) fn loading_body(done: u32, total: u32, org: &str) -> String {
    format!("Checking {done} of {total} repositories in {org}.")
}

pub(crate) fn no_prs_body(scope: &str) -> String {
    format!("Nobody you track has an open pull request in {scope}.")
}

pub(crate) fn no_match(query: &str) -> String {
    format!("No pull requests match “{query}”.")
}

pub(crate) const fn status_empty(status: PrStatus) -> &'static str {
    match status {
        PrStatus::NeedsReview => "Nothing needs review.",
        PrStatus::ActiveComments => "No active comments.",
        PrStatus::Reviewed => "Nothing reviewed yet.",
    }
}

pub(crate) fn signed_in_as(name: &str) -> String {
    format!("Signed in as {name}")
}

/// A one-word sign-in state for tooltips.
pub(crate) const fn auth_state_short(state: &prm_service::AuthStateView) -> &'static str {
    use prm_service::AuthStateView;
    match state {
        AuthStateView::SignedIn => "Signed in",
        AuthStateView::NeedsSignIn { .. } => "Sign-in needed",
        AuthStateView::Blocked { .. } => "Blocked",
        AuthStateView::Unknown | AuthStateView::Checking => "Checking sign-in",
        AuthStateView::SigningIn { .. } => "Signing in",
        AuthStateView::SignedOut | AuthStateView::Disabled => "Signed out",
    }
}

pub(crate) fn via(method: &str) -> String {
    format!("via {method}")
}

pub(crate) fn sign_in_needed(title: &str) -> String {
    format!("Sign-in needed: {title}")
}

pub(crate) fn blocked(title: &str) -> String {
    format!("Blocked: {title}")
}

pub(crate) fn pat_title(org: &str) -> String {
    format!("Use a personal access token for {org}")
}

pub(crate) fn remove_org_title(org: &str) -> String {
    format!("Remove {org}?")
}

pub(crate) fn whole_project(project: &str) -> String {
    format!("{project} (all repositories)")
}

pub(crate) fn step_of(step: usize, total: usize) -> String {
    format!("Step {step} of {total}")
}

pub(crate) fn repos_selected(k: usize, n: usize) -> String {
    format!("{k} of {n} repositories")
}

pub(crate) fn search_scope(projects: usize, repos: usize) -> String {
    format!("Search {projects} projects and {repos} repositories")
}

pub(crate) fn no_people(query: &str) -> String {
    format!("No people match “{query}”.")
}

/// The accessible name of a pill's remove button.
pub(crate) fn remove_person(name: &str) -> String {
    format!("Remove {name}")
}

pub(crate) fn test_failed(reason: &str) -> String {
    format!("Windows didn't show the notification: {reason}")
}

pub(crate) fn changes_not_saved(reason: &str) -> String {
    format!("Changes applied but couldn't be saved: {reason}")
}

pub(crate) fn match_windows(dark: &str, light: &str) -> String {
    format!("Match Windows (dark: {dark}, light: {light})")
}

pub(crate) fn using_renderer(api: &str, adapter: &str, software: bool) -> String {
    let suffix = if software {
        " (software rendering)"
    } else {
        ""
    };
    format!("Using {api} on {adapter}{suffix}")
}

pub(crate) fn more_banners(n: usize) -> String {
    format!("+{n} more")
}

/// The short note behind a change bar (also its tooltip).
pub(crate) fn change_short(kind: &ChangeKind) -> String {
    match kind {
        ChangeKind::NewPr => "New pull request".to_owned(),
        ChangeKind::Reactivated => "Reopened".to_owned(),
        ChangeKind::ReadyForReview => "Ready for review".to_owned(),
        ChangeKind::Closed { outcome } => match outcome {
            CloseOutcome::Completed => "Completed",
            CloseOutcome::Abandoned => "Abandoned",
            CloseOutcome::Unknown => "Closed",
        }
        .to_owned(),
        ChangeKind::Reviewed { headline } => vote_word(*headline).to_owned(),
        ChangeKind::NeedsReReview => "Needs review again".to_owned(),
        ChangeKind::ChangesRequested { by, vote } => {
            let verb = if *vote == VoteKind::Rejected {
                "rejected"
            } else {
                "is waiting for the author"
            };
            format!("{} {verb}", by.display_name())
        }
        ChangeKind::NewVote { by, .. } => format!("{} approved", by.display_name()),
        ChangeKind::NewComments { count, .. } => plural(*count, "new comment", "new comments"),
        ChangeKind::CommentsResolved => "All comments resolved".to_owned(),
        ChangeKind::NewPush => "New changes pushed".to_owned(),
    }
}

/// Button text for an action the service suggests.
pub(crate) fn user_action_label(action: &UserAction) -> &'static str {
    use prm_domain::app::DocLink;
    match action {
        UserAction::SignInWithMicrosoft => SIGN_IN_MICROSOFT,
        UserAction::SignInEntra => "Sign in",
        UserAction::EnterPat => USE_PAT,
        UserAction::ChooseSignIn => "Choose how to sign in",
        UserAction::Retry => "Retry",
        UserAction::ContactAdmin { .. } => "Copy message for your admin",
        UserAction::OpenDocs(DocLink::InstallAzureCli) => "Install Azure CLI",
        UserAction::OpenDocs(DocLink::CreatePat) => HOW_TO_CREATE_TOKEN,
        UserAction::OpenDocs(DocLink::EntraAppRegistration) => "How to register an app",
        UserAction::OpenDocs(DocLink::Troubleshooting) => "Troubleshooting",
        UserAction::OpenDocs(DocLink::ReportProblem) => REPORT_PROBLEM,
        UserAction::OpenSettings(SettingsSection::Scope) => "Review repositories",
        UserAction::OpenSettings(
            SettingsSection::General
            | SettingsSection::Organizations
            | SettingsSection::Auth
            | SettingsSection::People
            | SettingsSection::Notifications
            | SettingsSection::Appearance
            | SettingsSection::About,
        ) => "Open settings",
    }
}

pub(crate) const fn settings_section(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::General => "General",
        SettingsSection::Organizations
        | SettingsSection::Auth
        | SettingsSection::People
        | SettingsSection::Scope => "Organizations",
        SettingsSection::Notifications => "Notifications",
        SettingsSection::Appearance => "Appearance",
        SettingsSection::About => "About",
    }
}

/// Static strings the UI lays out as text, checked against the embedded fonts in tests.
#[cfg(test)]
pub(crate) const ALL: &[&str] = &[
    FILTER_HINT,
    NOT_UPDATED,
    UPDATED_NOW,
    SLOWED,
    THROTTLED,
    CHANGED_TOOLTIP,
    DRAFT,
    CONFLICTS,
    AUTO_COMPLETE,
    COLUMN_SQUEEZED,
    COLUMNS_HELP,
    REVIEWS,
    NO_OPEN_THREADS,
    SERVICE_STOPPED,
    EMPTY_LOADING,
    EMPTY_SIGNED_OUT,
    EMPTY_NO_PRS,
    NOTHING_CHANGED,
    NO_ORGS,
    BAD_ORG_URL,
    WAITING_SIGN_IN,
    PAT_HELP,
    ENTRA_HELP,
    REFRESH_HELP,
    REMOVE_ORG_BODY,
    REVIEWERS_HELP,
    TRACK_FROM_HELP,
    EVERYONE_BUT_ME,
    ANY_NON_AUTHOR,
    SEARCH_MIN,
    SEARCHING,
    SIGN_IN_TO_SEARCH,
    ADDING,
    GROUP_SUFFIX,
    PROJECTS_FAILED,
    SAVE_FAILED,
    TEST_SENT,
    ANIMATIONS_HELP,
    PRIVACY,
    PALETTE_HINT,
    PALETTE_FOOTER,
    TRAY_SIGN_IN,
];

#[cfg(test)]
mod tests;
