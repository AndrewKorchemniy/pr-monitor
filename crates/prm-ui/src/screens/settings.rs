//! Settings: a draft of the config edited in place and applied as a whole with Save. Sections
//! on the left; the footer shows whether there are unsaved changes.

pub(crate) mod about;
pub(crate) mod appearance;
pub(crate) mod general;
pub(crate) mod notifications;
pub(crate) mod orgs;

use std::{path::PathBuf, sync::Arc};

use eframe::egui::{self, RichText};
use prm_config::Config;
use prm_domain::auth::SettingsSection;
use prm_service::AppEvent;

use crate::{
    actions::UiAction,
    copy,
    deps::BuildInfo,
    frame::FrameCx,
    theme::icons::Icon,
    widgets::{self, controls, people_picker::PeoplePicker, scope_picker::ScopeFilter},
};

/// The pages in the sidebar (org sub-sections all open Organizations).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    General,
    Organizations,
    Notifications,
    Appearance,
    About,
}

pub(crate) const PAGES: [Page; 5] = [
    Page::General,
    Page::Organizations,
    Page::Notifications,
    Page::Appearance,
    Page::About,
];

impl Page {
    pub(crate) const fn from_section(section: SettingsSection) -> Self {
        match section {
            SettingsSection::General => Self::General,
            SettingsSection::Organizations
            | SettingsSection::Auth
            | SettingsSection::People
            | SettingsSection::Scope => Self::Organizations,
            SettingsSection::Notifications => Self::Notifications,
            SettingsSection::Appearance => Self::Appearance,
            SettingsSection::About => Self::About,
        }
    }

    pub(crate) const fn section(self) -> SettingsSection {
        match self {
            Self::General => SettingsSection::General,
            Self::Organizations => SettingsSection::Organizations,
            Self::Notifications => SettingsSection::Notifications,
            Self::Appearance => SettingsSection::Appearance,
            Self::About => SettingsSection::About,
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
    /// A save is in flight; the base updates when the service confirms.
    pub saving: bool,
    /// A save waits for the service's confirmation notice.
    pub confirm_pending: bool,
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
            saving: false,
            confirm_pending: false,
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.draft != *self.base
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
}

pub(crate) fn settings_screen(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    egui::Panel::left("settings_nav")
        .exact_size(200.0)
        .resizable(false)
        .frame(crate::theme::adapter::frame_chrome(&theme))
        .show(ui, |ui| nav(ui, cx, st));
    egui::Panel::bottom("settings_footer")
        .exact_size(48.0)
        .frame(crate::theme::adapter::frame_chrome(&theme))
        .show(ui, |ui| footer(ui, cx, st));
    egui::CentralPanel::default_margins().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.set_max_width(720.0);
                ui.add_space(theme.tokens.metrics.space.sm);
                // why: Organizations shows a breadcrumb with the selected org instead.
                if st.page != Page::Organizations {
                    widgets::heading(ui, &theme, st.page.title());
                    ui.add_space(theme.tokens.metrics.space.lg);
                }
                match st.page {
                    Page::General => general::page(ui, cx, &mut st.draft),
                    Page::Organizations => orgs::page(ui, cx, st),
                    Page::Notifications => notifications::page(ui, cx, &mut st.draft),
                    Page::Appearance => appearance::page(ui, cx, &mut st.draft, env),
                    Page::About => about::page(ui, cx, env),
                }
            });
    });
    orgs::editor(ui.ctx(), cx, st);
}

fn nav(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    let m = &theme.tokens.metrics;
    let width = ui.available_width();
    ui.add_space(m.space.md);
    if controls::icon_text_button(ui, &theme, Icon::ArrowLeft, copy::BACK, Some(width)).clicked() {
        cx.push(UiAction::LeaveSettings);
    }
    ui.add_space(m.space.lg);
    ui.spacing_mut().item_spacing.y = m.space.xxs;
    for page in PAGES {
        let text = RichText::new(page.title()).font(theme.fonts.body.clone());
        let button = egui::Button::selectable(st.page == page, text)
            .min_size(egui::vec2(width, m.row.control_height));
        if ui.add(button).clicked() {
            st.page = page;
        }
    }
}

fn footer(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &SettingsState) {
    let theme = cx.theme.clone();
    ui.horizontal_centered(|ui| {
        if st.dirty() {
            widgets::muted(ui, &theme, copy::UNSAVED);
        }
        let right = egui::Layout::right_to_left(egui::Align::Center);
        let enabled = st.dirty() && !st.saving;
        ui.with_layout(right, |ui| {
            ui.add_enabled_ui(enabled, |ui| footer_buttons(ui, cx))
        });
    });
}

fn footer_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>) {
    let theme = cx.theme.clone();
    if widgets::primary_button(ui, &theme, copy::SAVE_CHANGES).clicked() {
        cx.push(UiAction::SaveSettings);
    }
    if ui.button(copy::DISCARD).clicked() {
        cx.push(UiAction::DiscardSettings);
    }
}
