//! Settings: a draft of the config edited in place and applied as a whole with Save, shown in
//! a modal over the dashboard (see `shell`).

pub(crate) mod appearance;
pub(crate) mod general;
pub(crate) mod notifications;
pub(crate) mod orgs;
mod shell;

pub(crate) use shell::settings_modal;

use std::{path::PathBuf, sync::Arc};

use prm_config::Config;
use prm_domain::auth::SettingsSection;
use prm_service::AppEvent;

use crate::{
    copy,
    deps::BuildInfo,
    theme::icons::Icon,
    widgets::{people_picker::PeoplePicker, scope_picker::ScopeFilter},
};

/// The pages in the sidebar, or the page menu in a narrow window (org sub-sections all open
/// Organizations).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    General,
    Organizations,
    Notifications,
    Appearance,
}

pub(crate) const PAGES: [Page; 4] = [
    Page::General,
    Page::Organizations,
    Page::Notifications,
    Page::Appearance,
];

impl Page {
    pub(crate) const fn from_section(section: SettingsSection) -> Self {
        match section {
            // why: version, links and licenses live in the sidebar footer (at the end of General
            // when the sidebar is folded away), not on a page of their own.
            SettingsSection::General | SettingsSection::About => Self::General,
            SettingsSection::Organizations
            | SettingsSection::Auth
            | SettingsSection::People
            | SettingsSection::Scope => Self::Organizations,
            SettingsSection::Notifications => Self::Notifications,
            SettingsSection::Appearance => Self::Appearance,
        }
    }

    pub(crate) const fn section(self) -> SettingsSection {
        match self {
            Self::General => SettingsSection::General,
            Self::Organizations => SettingsSection::Organizations,
            Self::Notifications => SettingsSection::Notifications,
            Self::Appearance => SettingsSection::Appearance,
        }
    }

    pub(crate) const fn icon(self) -> Icon {
        match self {
            Self::General => Icon::Gear,
            Self::Organizations => Icon::Building,
            Self::Notifications => Icon::Bell,
            Self::Appearance => Icon::Contrast,
        }
    }

    pub(crate) const fn title(self) -> &'static str {
        copy::settings_section(self.section())
    }
}

/// A dialog open over the Organizations page.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Editor {
    AddOrg,
    /// Third-party licenses, from the about links.
    Licenses,
    Repositories,
    Tracked,
    Reviewers,
    Ignored,
}

/// People pickers per org page: tracked authors, reviewers, ignored comment authors.
pub(crate) struct Pickers {
    pub tracked: PeoplePicker,
    pub reviewers: PeoplePicker,
    pub ignored: PeoplePicker,
}

impl Default for Pickers {
    fn default() -> Self {
        Self {
            tracked: PeoplePicker::new("tracked"),
            reviewers: PeoplePicker::new("reviewers"),
            ignored: PeoplePicker::new("ignored"),
        }
    }
}

pub(crate) struct SettingsState {
    pub base: Arc<Config>,
    pub draft: Config,
    pub page: Page,
    pub org: usize,
    pub pickers: Pickers,
    pub scope_filter: ScopeFilter,
    pub new_org_url: String,
    pub url_error: bool,
    pub editing: Option<Editor>,
    /// A section to scroll into view on the next frame ("Edit tracked people" opens People).
    pub scroll_to: Option<SettingsSection>,
    /// A save is in flight; the base updates when the service confirms.
    pub saving: bool,
    /// A save waits for the service to say whether the settings file was written.
    pub confirm_pending: bool,
    /// The last save was applied but not written; the Save bar shows it until a write succeeds.
    pub save_error: Option<Arc<str>>,
    /// The last test notification, shown beside its button.
    pub test_result: notifications::TestResult,
    /// Scroll the test button into view on the next frame (a test sent from the palette).
    pub reveal_test: bool,
}

impl SettingsState {
    pub(crate) fn new(base: Arc<Config>, section: SettingsSection) -> Self {
        Self {
            draft: (*base).clone(),
            base,
            page: Page::from_section(section),
            org: 0,
            pickers: Pickers::default(),
            scope_filter: ScopeFilter::default(),
            new_org_url: String::new(),
            url_error: false,
            editing: None,
            scroll_to: Some(section),
            saving: false,
            confirm_pending: false,
            save_error: None,
            test_result: None,
            reveal_test: false,
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.draft != *self.base
    }

    /// The Save bar shows: unsaved edits, or a save that could not be written (to retry).
    pub(crate) fn save_bar_shown(&self) -> bool {
        self.dirty() || self.save_error.is_some()
    }

    /// Routes people search results to the picker that asked; adds resolved people.
    pub(crate) fn on_event(&mut self, event: &AppEvent) {
        let Some(org) = self.draft.orgs.get_mut(self.org) else {
            return;
        };
        let people = &mut org.people;
        for (picker, list) in [
            (&mut self.pickers.tracked, &mut people.tracked_authors),
            (&mut self.pickers.reviewers, &mut people.reviewers),
            (
                &mut self.pickers.ignored,
                &mut people.ignored_comment_authors,
            ),
        ] {
            if let Some(person) = picker.on_event(event)
                && !list.iter().any(|p| p.id == person.id)
            {
                list.push(person);
            }
        }
    }

    /// The service applied a config: it becomes the new base (keeping unrelated edits).
    pub(crate) fn on_applied(&mut self, applied: Arc<Config>) {
        if self.saving {
            self.draft = (*applied).clone();
            self.saving = false;
        }
        self.base = applied;
    }
}

/// What settings needs besides the draft.
pub(crate) struct SettingsEnv<'a> {
    /// (id, display name) of every installed theme.
    pub themes: Vec<(String, String)>,
    pub themes_dir: PathBuf,
    /// Problems with theme files ("midnight: row.selected fails contrast"), shown on Appearance.
    pub theme_issues: Vec<String>,
    pub build: BuildInfo,
    pub licenses: &'static str,
    pub renderer_line: &'a str,
    /// The table's column choices, live: they are layout (`UiState`), applied right away
    /// rather than through the draft.
    pub columns: crate::view::columns::Choices<'a>,
}
