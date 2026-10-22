//! Organizations: a breadcrumb and tabs for the selected org, its connection, details,
//! repositories and people in sections, and dialogs for adding an org and editing its lists.

mod dialogs;
mod sign_in;
mod tabs;

pub(crate) use dialogs::editor;
use eframe::egui::{self, RichText};
use prm_config::OrgProfile;
use prm_domain::{
    OrgId, Person, auth::SettingsSection, classify::EmptyReviewerPolicy, ids::parse_org_input,
    plan::DraftPolicy,
};
use prm_service::Command;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    route::ModalKind,
    screens::settings::{Editor, SettingsState},
    theme::UiTheme,
    widgets::{
        self, card,
        controls::{self, setting_row, toggle_row},
        fields::{self, Field},
        fit, people_picker,
        scope_picker::{self, CatalogState},
    },
};

/// A profile for an org URL as typed by the user.
pub(crate) fn new_profile(input: &str) -> Option<OrgProfile> {
    let org = parse_org_input(input).ok()?;
    Some(OrgProfile {
        url: format!("https://dev.azure.com/{}", org.as_str()),
        label: org.as_str().to_owned(),
        id: Some(org),
        ..OrgProfile::default()
    })
}

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    tabs::org_tabs(ui, cx, st);
    ui.add_space(theme.tokens.metrics.space.xl);
    if st.org >= st.draft.orgs.len() {
        widgets::muted(ui, &theme, copy::NO_ORGS);
        return;
    }
    let saved = saved(st);
    super::reveal(ui, st, &[SettingsSection::Auth]);
    card::section(ui, &theme, copy::CONNECTION, None, |ui| {
        sign_in::section(ui, cx, st, saved);
    });
    card::section(ui, &theme, copy::ORGANIZATION, None, |ui| {
        details(ui, &theme, &mut st.draft.orgs[st.org]);
    });
    super::reveal(ui, st, &[SettingsSection::Scope]);
    card::section(ui, &theme, copy::REPOSITORIES, None, |ui| {
        repositories(ui, cx, st);
    });
    super::reveal(ui, st, &[SettingsSection::People]);
    card::section(ui, &theme, copy::PEOPLE, None, |ui| people(ui, &theme, st));
    // why: set apart from the people lists above and in the danger tone, like its confirmation.
    ui.add_space(theme.tokens.metrics.space.xl);
    if widgets::danger_button(ui, &theme, copy::REMOVE_ORG).clicked() {
        cx.push(UiAction::Modal(ModalKind::RemoveOrg(st.org)));
    }
}

/// The selected org is in the saved config (so it can sign in and search people).
fn saved(st: &SettingsState) -> bool {
    st.draft
        .orgs
        .get(st.org)
        .and_then(OrgProfile::org_id)
        .is_some_and(|id| st.base.org(id).is_some())
}

/// "Organizations › contoso", the page title in the settings header (lays out inline).
pub(crate) fn breadcrumb(ui: &mut egui::Ui, theme: &UiTheme, st: &SettingsState) {
    let current = st.draft.orgs.get(st.org).map(OrgProfile::display_label);
    let mut crumbs = vec![copy::settings_section(SettingsSection::Organizations)];
    if let Some(label) = current.as_deref() {
        crumbs.push(label);
    }
    ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.sm;
    card::breadcrumb(ui, theme, &crumbs);
}

/// Adds (or selects) the org typed in the field. A new org is applied to the service right away
/// (on top of the saved config, not the unsaved edits) so it can sign in before Save.
fn add_org(st: &mut SettingsState, cx: &mut FrameCx<'_>) -> bool {
    let Some(profile) = new_profile(&st.new_org_url) else {
        st.url_error = true;
        return false;
    };
    let existing = st
        .draft
        .orgs
        .iter()
        .position(|o| o.org_id() == profile.id.as_ref());
    st.org = existing.unwrap_or_else(|| {
        let mut applied = (*st.base).clone();
        applied.orgs.push(profile.clone());
        cx.push(UiAction::Send(Command::ApplyConfig(Box::new(applied))));
        st.draft.orgs.push(profile);
        st.draft.orgs.len() - 1
    });
    st.new_org_url.clear();
    st.url_error = false;
    st.pickers = super::Pickers::default();
    true
}

fn details(ui: &mut egui::Ui, theme: &UiTheme, org: &mut OrgProfile) {
    setting_row(ui, theme, (copy::NAME, Some(&org.url)), |ui| {
        // why: the hint is also the field's accessible name.
        let field =
            Field::new(ui.make_persistent_id("org_name"), copy::NAME).width(fit::CONTROL_WIDTH);
        fields::text_field(ui, theme, &mut org.label, &field);
    });
    controls::row_divider(ui, theme);
    toggle_row(ui, theme, &mut org.enabled, (copy::WATCH_ORG, None));
    controls::row_divider(ui, theme);
    toggle_row(ui, theme, &mut org.muted, (copy::MUTE_ORG, None));
}

fn repositories(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    let state = cx.state;
    let org = &mut st.draft.orgs[st.org];
    let id: Option<OrgId> = org.org_id().cloned();
    let catalog = CatalogState::of(id.as_ref().and_then(|id| state.org(id)));
    if matches!(catalog, CatalogState::Loading)
        && let Some(id) = &id
    {
        request_catalog(ui, cx, id, false);
    }
    let summary = scope_picker::summary(catalog, &org.scope);
    let detail = scope_picker::detail(&org.scope);
    if row_with_button(ui, &theme, (&summary, Some(&detail)), copy::CHOOSE) {
        st.editing = Some(Editor::Repositories);
    }
    controls::row_divider(ui, &theme);
    let mut drafts = org.drafts == DraftPolicy::Show;
    if toggle_row(ui, &theme, &mut drafts, (copy::INCLUDE_DRAFTS, None)).changed() {
        org.drafts = if drafts {
            DraftPolicy::Show
        } else {
            DraftPolicy::Hide
        };
    }
    controls::row_divider(ui, &theme);
    toggle_row(
        ui,
        &theme,
        &mut org.org_wide_query,
        (copy::ORG_WIDE_QUERY, None),
    );
}

/// A title (and an optional muted line) on the left, a button on the right; true when clicked.
/// The button goes first, so the text wraps short of it in a narrow window.
fn row_with_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: (&str, Option<&str>),
    button: &str,
) -> bool {
    let space = theme.tokens.metrics.space.sm;
    ui.add_space(space);
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clicked = ui.button(button).clicked();
            let column = egui::Layout::top_down(egui::Align::Min);
            ui.with_layout(column, |ui| title_and_detail(ui, theme, text));
        });
    });
    ui.add_space(space);
    clicked
}

fn title_and_detail(ui: &mut egui::Ui, theme: &UiTheme, (title, detail): (&str, Option<&str>)) {
    ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        widgets::muted(ui, theme, detail);
    }
}

/// Asks for the catalog once per org per settings visit.
fn request_catalog(ui: &egui::Ui, cx: &mut FrameCx<'_>, org: &OrgId, force: bool) {
    let id = egui::Id::unique(("catalog_requested", org.as_str()));
    let asked = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if !asked {
        ui.data_mut(|d| d.insert_temp(id, true));
        cx.push(UiAction::Send(Command::LoadCatalog {
            org: org.clone(),
            force,
        }));
    }
}

/// The people lists the People section shows, in order.
const PEOPLE_LISTS: [Editor; 3] = [Editor::Tracked, Editor::Reviewers, Editor::Ignored];

/// A people list's title and help, in the People section and in its edit dialog.
const fn list_copy(editor: Editor) -> (&'static str, &'static str) {
    match editor {
        Editor::Reviewers => (copy::REVIEWERS_COUNT, copy::REVIEWERS_HELP),
        Editor::Ignored => (copy::IGNORE_FROM, copy::IGNORE_HELP),
        Editor::Tracked | Editor::AddOrg | Editor::Licenses | Editor::Repositories => {
            (copy::TRACK_FROM, copy::TRACK_FROM_HELP)
        }
    }
}

fn people(ui: &mut egui::Ui, theme: &UiTheme, st: &mut SettingsState) {
    let org = &mut st.draft.orgs[st.org];
    for (index, editor) in PEOPLE_LISTS.into_iter().enumerate() {
        if index > 0 {
            controls::row_divider(ui, theme);
        }
        let (title, help) = list_copy(editor);
        if row_with_button(ui, theme, (title, Some(help)), copy::EDIT) {
            st.editing = Some(editor);
        }
        let when_empty = empty_list_meaning(editor, org.include_me);
        let list = people_list(org, editor);
        if list.is_empty() {
            people_picker::placeholder(ui, theme, when_empty);
        } else {
            people_picker::chips(ui, theme, list);
        }
        match editor {
            Editor::Tracked => {
                toggle_row(ui, theme, &mut org.include_me, (copy::INCLUDE_ME, None));
            }
            Editor::Reviewers if org.people.reviewers.is_empty() => {
                reviewer_policy(ui, theme, &mut org.review.empty_reviewer_list);
            }
            Editor::Reviewers
            | Editor::Ignored
            | Editor::AddOrg
            | Editor::Licenses
            | Editor::Repositories => {}
        }
    }
}

/// What an empty list shows: an empty tracked list tracks everyone in scope.
const fn empty_list_meaning(editor: Editor, include_me: bool) -> &'static str {
    match editor {
        Editor::Tracked if include_me => copy::EVERYONE,
        Editor::Tracked => copy::EVERYONE_BUT_ME,
        Editor::Reviewers
        | Editor::Ignored
        | Editor::AddOrg
        | Editor::Licenses
        | Editor::Repositories => copy::NOBODY_YET,
    }
}

fn reviewer_policy(ui: &mut egui::Ui, theme: &UiTheme, policy: &mut EmptyReviewerPolicy) {
    widgets::field_label(ui, theme, copy::EMPTY_LIST_POLICY);
    let any = EmptyReviewerPolicy::AnyNonAuthorVote;
    controls::radio(ui, theme, policy, any, copy::ANY_NON_AUTHOR);
    controls::radio(
        ui,
        theme,
        policy,
        EmptyReviewerPolicy::Nobody,
        copy::NO_VOTES,
    );
}

/// The list a people editor edits (tracked authors for the non-people editors).
fn people_list(org: &mut OrgProfile, editor: Editor) -> &mut Vec<Person> {
    let people = &mut org.people;
    match editor {
        Editor::Reviewers => &mut people.reviewers,
        Editor::Ignored => &mut people.ignored_comment_authors,
        Editor::Tracked | Editor::AddOrg | Editor::Licenses | Editor::Repositories => {
            &mut people.tracked_authors
        }
    }
}
