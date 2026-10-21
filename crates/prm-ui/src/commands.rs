//! The command registry: one list drives the palette, keyboard shortcuts, buttons, context
//! menus and the tray (menu ids are command keys).

pub(crate) mod keymap;
pub(crate) mod palette;

use std::time::Duration;

/// How long "Pause notifications" lasts. The pause labels say "1 hour"; a test keeps them in
/// step with this.
pub(crate) const PAUSE: Duration = Duration::from_hours(1);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CommandId {
    PaletteOpen,
    FilterFocus,
    RefreshNow,
    SettingsOpen,
    ShortcutsShow,
    StatusAll,
    StatusNeedsReview,
    StatusActiveComments,
    StatusReviewed,
    ChangedOnly,
    HideDrafts,
    GroupCycle,
    GroupNone,
    GroupStatus,
    GroupAuthor,
    GroupRepo,
    GroupOrg,
    GroupProject,
    SortStatus,
    SortNumber,
    SortTitle,
    SortRepo,
    SortAuthor,
    SortAge,
    SortActivity,
    SortComments,
    NavDown,
    NavUp,
    NavFirst,
    NavLast,
    NavPageDown,
    NavPageUp,
    NavNextGroup,
    NavPrevGroup,
    GroupCollapse,
    GroupExpand,
    PrOpen,
    PrPeek,
    PrCopyLink,
    PrCopyMarkdown,
    PrCopyTitle,
    PrFilterAuthor,
    PrFilterRepo,
    PrMarkSeen,
    PrMarkAllSeen,
    NotificationsPause,
    NotificationsResume,
    NotificationsTest,
    AuthMenu,
    WindowShow,
    WindowHide,
    AppQuit,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ThemeCycle,
    ColumnsReset,
    AboutOpen,
    SettingsSave,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    General,
    View,
    Navigate,
    PullRequest,
    Notifications,
    Window,
}

/// Where a shortcut applies. Global shortcuts need a modifier or a function key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    Global,
    Table,
    Settings,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandSpec {
    pub id: CommandId,
    /// Stable id: keymap.toml, tray menu ids.
    pub key: &'static str,
    pub title: &'static str,
    pub category: Category,
    pub keys: &'static [(&'static str, Scope)],
    /// Shown in the command palette.
    pub palette: bool,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a const constructor for the command table"
)]
const fn cmd(
    id: CommandId,
    key: &'static str,
    title: &'static str,
    category: Category,
    keys: &'static [(&'static str, Scope)],
    palette: bool,
) -> CommandSpec {
    CommandSpec {
        id,
        key,
        title,
        category,
        keys,
        palette,
    }
}

use Category::{General, Navigate, Notifications, PullRequest, View, Window};
use CommandId as C;
use Scope::{Global as G, Settings as S, Table as T};

pub(crate) static COMMANDS: &[CommandSpec] = &[
    cmd(
        C::PaletteOpen,
        "palette.open",
        "Open command palette",
        General,
        &[("Ctrl+K", G), ("Ctrl+Shift+P", G)],
        false,
    ),
    cmd(
        C::FilterFocus,
        "filter.focus",
        "Filter pull requests",
        View,
        &[("/", T), ("Ctrl+F", G)],
        true,
    ),
    cmd(
        C::RefreshNow,
        "refresh.now",
        "Refresh now",
        General,
        &[("F5", G), ("Ctrl+R", G)],
        true,
    ),
    cmd(
        C::SettingsOpen,
        "settings.open",
        "Open settings",
        General,
        &[("Ctrl+Comma", G)],
        true,
    ),
    cmd(
        C::ShortcutsShow,
        "shortcuts.show",
        "Show keyboard shortcuts",
        General,
        &[("?", T), ("F1", G)],
        true,
    ),
    cmd(
        C::StatusAll,
        "view.status_all",
        "Show all statuses",
        View,
        &[("0", T)],
        true,
    ),
    cmd(
        C::StatusNeedsReview,
        "view.needs_review",
        "Show only Needs review",
        View,
        &[("1", T)],
        true,
    ),
    cmd(
        C::StatusActiveComments,
        "view.active_comments",
        "Show only Active comments",
        View,
        &[("2", T)],
        true,
    ),
    cmd(
        C::StatusReviewed,
        "view.reviewed",
        "Show only Reviewed",
        View,
        &[("3", T)],
        true,
    ),
    cmd(
        C::ChangedOnly,
        "view.changed_only",
        "Show only changed pull requests",
        View,
        &[("C", T)],
        true,
    ),
    cmd(
        C::HideDrafts,
        "view.hide_drafts",
        "Hide drafts",
        View,
        &[("D", T)],
        true,
    ),
    cmd(
        C::GroupCycle,
        "view.group_cycle",
        "Change grouping",
        View,
        &[("Ctrl+G", G)],
        true,
    ),
    cmd(
        C::GroupNone,
        "view.group.none",
        "Don't group",
        View,
        &[],
        true,
    ),
    cmd(
        C::GroupStatus,
        "view.group.status",
        "Group by status",
        View,
        &[],
        true,
    ),
    cmd(
        C::GroupAuthor,
        "view.group.author",
        "Group by author",
        View,
        &[],
        true,
    ),
    cmd(
        C::GroupRepo,
        "view.group.repo",
        "Group by repository",
        View,
        &[],
        true,
    ),
    cmd(
        C::GroupOrg,
        "view.group.org",
        "Group by organization",
        View,
        &[],
        true,
    ),
    cmd(
        C::GroupProject,
        "view.group.project",
        "Group by project",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortStatus,
        "view.sort.status",
        "Sort by status",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortNumber,
        "view.sort.number",
        "Sort by number",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortTitle,
        "view.sort.title",
        "Sort by title",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortRepo,
        "view.sort.repo",
        "Sort by repository",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortAuthor,
        "view.sort.author",
        "Sort by author",
        View,
        &[],
        true,
    ),
    cmd(C::SortAge, "view.sort.age", "Sort by age", View, &[], true),
    cmd(
        C::SortActivity,
        "view.sort.activity",
        "Sort by last activity",
        View,
        &[],
        true,
    ),
    cmd(
        C::SortComments,
        "view.sort.comments",
        "Sort by comments",
        View,
        &[],
        true,
    ),
    cmd(
        C::NavDown,
        "nav.down",
        "Next pull request",
        Navigate,
        &[("J", T), ("ArrowDown", T)],
        false,
    ),
    cmd(
        C::NavUp,
        "nav.up",
        "Previous pull request",
        Navigate,
        &[("K", T), ("ArrowUp", T)],
        false,
    ),
    cmd(
        C::NavFirst,
        "nav.first",
        "First pull request",
        Navigate,
        &[("Home", T)],
        false,
    ),
    cmd(
        C::NavLast,
        "nav.last",
        "Last pull request",
        Navigate,
        &[("End", T)],
        false,
    ),
    cmd(
        C::NavPageDown,
        "nav.page_down",
        "Page down",
        Navigate,
        &[("PageDown", T)],
        false,
    ),
    cmd(
        C::NavPageUp,
        "nav.page_up",
        "Page up",
        Navigate,
        &[("PageUp", T)],
        false,
    ),
    cmd(
        C::NavNextGroup,
        "nav.next_group",
        "Next group",
        Navigate,
        &[("Ctrl+ArrowDown", T)],
        false,
    ),
    cmd(
        C::NavPrevGroup,
        "nav.prev_group",
        "Previous group",
        Navigate,
        &[("Ctrl+ArrowUp", T)],
        false,
    ),
    cmd(
        C::GroupCollapse,
        "group.collapse",
        "Collapse group",
        Navigate,
        &[("ArrowLeft", T)],
        false,
    ),
    cmd(
        C::GroupExpand,
        "group.expand",
        "Expand group",
        Navigate,
        &[("ArrowRight", T)],
        false,
    ),
    cmd(
        C::PrOpen,
        "pr.open",
        "Open in browser",
        PullRequest,
        &[("Enter", T), ("O", T)],
        true,
    ),
    cmd(
        C::PrPeek,
        "pr.peek",
        "Peek details",
        PullRequest,
        &[("Space", T)],
        true,
    ),
    cmd(
        C::PrCopyLink,
        "pr.copy_link",
        "Copy link",
        PullRequest,
        &[("Ctrl+C", T)],
        true,
    ),
    cmd(
        C::PrCopyMarkdown,
        "pr.copy_markdown",
        "Copy Markdown link",
        PullRequest,
        &[("Ctrl+Shift+C", T)],
        true,
    ),
    cmd(
        C::PrCopyTitle,
        "pr.copy_title",
        "Copy title",
        PullRequest,
        &[],
        true,
    ),
    cmd(
        C::PrFilterAuthor,
        "pr.filter_author",
        "Filter to this author",
        PullRequest,
        &[("Ctrl+Enter", T)],
        true,
    ),
    cmd(
        C::PrFilterRepo,
        "pr.filter_repo",
        "Filter to this repository",
        PullRequest,
        &[("Ctrl+Shift+Enter", T)],
        true,
    ),
    cmd(
        C::PrMarkSeen,
        "pr.mark_seen",
        "Mark as seen",
        PullRequest,
        &[("M", T)],
        true,
    ),
    cmd(
        C::PrMarkAllSeen,
        "pr.mark_all_seen",
        "Mark all as seen",
        PullRequest,
        &[("Shift+M", T)],
        true,
    ),
    cmd(
        C::NotificationsPause,
        "notifications.pause",
        "Pause notifications for 1 hour",
        Notifications,
        &[],
        true,
    ),
    cmd(
        C::NotificationsResume,
        "notifications.resume",
        "Resume notifications",
        Notifications,
        &[],
        true,
    ),
    cmd(
        C::NotificationsTest,
        "notifications.test",
        "Send a test notification",
        Notifications,
        &[],
        true,
    ),
    cmd(C::AuthMenu, "auth.menu", "Sign in…", General, &[], true),
    cmd(
        C::WindowShow,
        "window.show",
        "Open PR Monitor",
        Window,
        &[],
        false,
    ),
    cmd(
        C::WindowHide,
        "window.hide",
        "Hide to tray",
        Window,
        &[("Ctrl+W", G)],
        true,
    ),
    cmd(
        C::AppQuit,
        "app.quit",
        "Quit PR Monitor",
        Window,
        &[("Ctrl+Q", G)],
        true,
    ),
    cmd(
        C::ZoomIn,
        "zoom.in",
        "Larger text",
        View,
        &[("Ctrl+Equals", G), ("Ctrl+Plus", G)],
        true,
    ),
    cmd(
        C::ZoomOut,
        "zoom.out",
        "Smaller text",
        View,
        &[("Ctrl+Minus", G)],
        true,
    ),
    cmd(
        C::ZoomReset,
        "zoom.reset",
        "Reset text size",
        View,
        &[("Ctrl+0", G)],
        true,
    ),
    cmd(C::ThemeCycle, "theme.cycle", "Next theme", View, &[], true),
    cmd(
        C::ColumnsReset,
        "columns.reset",
        "Reset columns",
        View,
        &[],
        true,
    ),
    cmd(
        C::AboutOpen,
        "about.open",
        "About PR Monitor",
        General,
        &[],
        true,
    ),
    cmd(
        C::SettingsSave,
        "settings.save",
        "Save changes",
        General,
        &[("Ctrl+S", S)],
        false,
    ),
];

impl CommandId {
    pub(crate) fn spec(self) -> &'static CommandSpec {
        COMMANDS
            .iter()
            .find(|c| c.id == self)
            .unwrap_or(&COMMANDS[0])
    }

    pub(crate) fn key(self) -> &'static str {
        self.spec().key
    }

    pub(crate) fn title(self) -> &'static str {
        self.spec().title
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        COMMANDS.iter().find(|c| c.key == key).map(|c| c.id)
    }
}

#[cfg(test)]
mod tests;
