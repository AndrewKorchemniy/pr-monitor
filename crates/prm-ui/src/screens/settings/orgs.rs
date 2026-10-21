//! Organizations: a breadcrumb and tabs for the selected org, its connection, details,
//! repositories and people in sections, and dialogs for adding an org and editing its lists.

mod sign_in;

use eframe::egui::{self, RichText};
use prm_config::OrgProfile;
use prm_domain::{
    OrgId, Person, auth::SettingsSection, classify::EmptyReviewerPolicy, ids::parse_org_input,
    plan::DraftPolicy,
};
use prm_service::{AuthStateView, Command};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    route::ModalKind,
    screens::settings::{Editor, SettingsEnv, SettingsState},
    theme::{Tone, UiTheme, icons::Icon},
    widgets::{
        self, card,
        controls::{self, setting_row, toggle_row},
        fields::{self, Field},
        modal::{self, ModalOutcome, ModalSpec},
        people_picker::{self, people_picker},
        scope_picker::{self, CatalogState, scope_picker},
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
    org_tabs(ui, cx, st);
    ui.add_space(theme.tokens.metrics.space.xl);
    if st.org >= st.draft.orgs.len() {
        widgets::muted(ui, &theme, copy::NO_ORGS);
        return;
    }
    let saved = saved(st);
    reveal(ui, st, &[SettingsSection::Auth]);
    card::section(ui, &theme, copy::CONNECTION, None, |ui| {
        sign_in::section(ui, cx, st, saved);
    });
    card::section(ui, &theme, copy::ORGANIZATION, None, |ui| {
        details(ui, &theme, &mut st.draft.orgs[st.org]);
    });
    reveal(ui, st, &[SettingsSection::Scope]);
    card::section(ui, &theme, copy::REPOSITORIES, None, |ui| {
        repositories(ui, cx, st);
    });
    reveal(ui, st, &[SettingsSection::People]);
    card::section(ui, &theme, copy::PEOPLE, None, |ui| people(ui, &theme, st));
    if ui.button(copy::REMOVE_ORG).clicked() {
        cx.push(UiAction::Modal(ModalKind::RemoveOrg(st.org)));
    }
}

/// Scrolls the section starting here into view when settings opened for it.
fn reveal(ui: &mut egui::Ui, st: &mut SettingsState, sections: &[SettingsSection]) {
    if st.scroll_to.is_some_and(|s| sections.contains(&s)) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
        st.scroll_to = None;
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

/// One tab per org with its sign-in state as a dot, then Add organization, over a baseline.
fn org_tabs(ui: &mut egui::Ui, cx: &FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    let mut picked = None;
    let mut add = false;
    let row = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.xs;
        for (index, org) in st.draft.orgs.iter().enumerate() {
            let state = org
                .org_id()
                .and_then(|id| cx.state.org(id))
                .map(|o| &o.auth.state);
            let label = org.display_label();
            if org_tab(ui, &theme, (&label, state), st.org == index).clicked() {
                picked = Some(index);
            }
        }
        ui.add_space(theme.tokens.metrics.space.sm);
        add = add_tab(ui, &theme).clicked();
    });
    let line = row.response.rect;
    let stroke = egui::Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter()
        .hline(line.left()..=ui.max_rect().right(), line.bottom(), stroke);
    if add {
        st.new_org_url.clear();
        st.url_error = false;
        st.editing = Some(Editor::AddOrg);
    }
    if let Some(index) = picked
        && index != st.org
    {
        st.org = index;
        st.pickers = super::Pickers::default();
    }
}

const fn auth_tone(state: &AuthStateView) -> Tone {
    match state {
        AuthStateView::SignedIn => Tone::Success,
        AuthStateView::NeedsSignIn { .. } => Tone::Warning,
        AuthStateView::Blocked { .. } => Tone::Danger,
        AuthStateView::Unknown
        | AuthStateView::Checking
        | AuthStateView::SigningIn { .. }
        | AuthStateView::SignedOut
        | AuthStateView::Disabled => Tone::Neutral,
    }
}

fn org_tab(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (label, state): (&str, Option<&AuthStateView>),
    selected: bool,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let c = &theme.colors;
    let (text, font) = if selected {
        (c.text_primary, theme.fonts.body_strong.clone())
    } else {
        (c.text_secondary, theme.fonts.body.clone())
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, text);
    let pad = m.space.lg;
    let width = pad + 7.0 + m.space.sm + galley.size().x + pad;
    let size = egui::vec2(width, theme.control_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.hovered() && !selected {
        let top = egui::CornerRadius {
            nw: m.radius.md,
            ne: m.radius.md,
            sw: 0,
            se: 0,
        };
        ui.painter().rect_filled(rect, top, c.control_hover);
    }
    if selected {
        let bar = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 2.0), rect.max);
        ui.painter().rect_filled(bar, 1.0, c.accent_base);
    }
    let tone = state.map_or(Tone::Neutral, auth_tone);
    let dot = egui::pos2(rect.left() + pad + 3.5, rect.center().y);
    ui.painter().circle_filled(dot, 3.5, c.tone(tone).solid);
    let at = egui::pos2(
        dot.x + 3.5 + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, text);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, label));
    match state.map(copy::auth_state_short) {
        Some(tip) => response.on_hover_text(tip),
        None => response,
    }
}

/// "+ Add organization" drawn like a tab, so the strip reads as one control.
fn add_tab(ui: &mut egui::Ui, theme: &UiTheme) -> egui::Response {
    let m = &theme.tokens.metrics;
    let galley = ui.painter().layout_no_wrap(
        copy::ADD_ORG.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_secondary,
    );
    let pad = m.space.lg;
    let width = pad + 12.0 + m.space.sm + galley.size().x + pad;
    let size = egui::vec2(width, theme.control_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    controls::ghost_fill(ui, theme, rect, &response);
    let color = if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };
    let glyph = egui::Rect::from_center_size(
        egui::pos2(rect.left() + pad + 6.0, rect.center().y),
        egui::vec2(12.0, 12.0),
    );
    crate::theme::icons::paint(ui.painter(), glyph, Icon::Plus, color);
    let at = egui::pos2(
        glyph.right() + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    let name = copy::ADD_ORG;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response
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
        let field = Field::new(ui.make_persistent_id("org_name"), copy::NAME).width(280.0);
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
fn row_with_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (title, detail): (&str, Option<&str>),
    button: &str,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
            if let Some(detail) = detail.filter(|d| !d.is_empty()) {
                widgets::muted(ui, theme, detail);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clicked = ui.button(button).clicked();
        });
    });
    clicked
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

/// The people lists the People section shows, with their titles and help.
const PEOPLE_LISTS: [(Editor, &str, Option<&str>); 3] = [
    (Editor::Tracked, copy::TRACK_FROM, None),
    (
        Editor::Reviewers,
        copy::REVIEWERS_COUNT,
        Some(copy::REVIEWERS_HELP),
    ),
    (Editor::Ignored, copy::IGNORE_FROM, Some(copy::IGNORE_HELP)),
];

fn people(ui: &mut egui::Ui, theme: &UiTheme, st: &mut SettingsState) {
    let org = &mut st.draft.orgs[st.org];
    for (index, (editor, title, help)) in PEOPLE_LISTS.into_iter().enumerate() {
        if index > 0 {
            controls::row_divider(ui, theme);
        }
        if row_with_button(ui, theme, (title, help), copy::EDIT) {
            st.editing = Some(editor);
        }
        let list = people_list(org, editor);
        if list.is_empty() {
            widgets::muted(ui, theme, copy::NOBODY_YET);
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

/// The open dialog over the page, if any: add an org, choose repositories, or edit people.
pub(crate) fn editor(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let Some(editor) = st.editing else {
        return;
    };
    let outcome = match editor {
        Editor::AddOrg => add_org_dialog(ctx, cx, st),
        Editor::Licenses => licenses_dialog(ctx, &cx.theme.clone(), env),
        Editor::Repositories => repositories_dialog(ctx, cx, st),
        Editor::Tracked | Editor::Reviewers | Editor::Ignored => people_dialog(ctx, cx, st, editor),
    };
    if outcome != ModalOutcome::Open {
        st.editing = None;
    }
}

/// Third-party licenses and the bundled fonts' licenses, in a scrolling dialog.
fn licenses_dialog(ctx: &egui::Context, theme: &UiTheme, env: &SettingsEnv<'_>) -> ModalOutcome {
    let spec = ModalSpec {
        id: "licenses",
        title: copy::LICENSES,
        subtitle: None,
        buttons: &[copy::CLOSE],
        danger: false,
        width: 640.0,
    };
    modal::modal(ctx, theme, &spec, |ui| {
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for text in [
                    env.licenses,
                    crate::theme::fonts::OFL_NEXT,
                    crate::theme::fonts::OFL_MONO,
                ] {
                    ui.label(
                        RichText::new(text)
                            .font(theme.fonts.mono.clone())
                            .color(theme.colors.text_secondary),
                    );
                    controls::row_divider(ui, theme);
                }
            });
    })
}

fn add_org_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
) -> ModalOutcome {
    let theme = &cx.theme.clone();
    let spec = ModalSpec {
        id: "add_org",
        title: copy::ADD_ORG,
        subtitle: None,
        buttons: &[],
        danger: false,
        width: modal::WIDTH,
    };
    let mut added = false;
    let outcome = modal::modal(ctx, theme, &spec, |ui| {
        widgets::field_label(ui, theme, copy::ORG_URL_LABEL);
        ui.add_space(theme.tokens.metrics.space.xs);
        let mut field = Field::new(egui::Id::unique("add_org_url"), copy::ORG_URL_HINT);
        field.error = st.url_error;
        let action = (&field, copy::ADD_ORG, Icon::ArrowRight);
        if fields::action_field(ui, theme, &mut st.new_org_url, action)
            && !st.new_org_url.trim().is_empty()
        {
            added = add_org(st, cx);
        }
        if st.url_error {
            widgets::error_text(ui, theme, copy::BAD_ORG_URL);
        }
    });
    if added {
        ModalOutcome::Button(0)
    } else {
        outcome
    }
}

fn repositories_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
) -> ModalOutcome {
    let theme = cx.theme.clone();
    let org_label = st
        .draft
        .orgs
        .get(st.org)
        .map(|o| o.display_label().to_string());
    let spec = ModalSpec {
        id: "choose_repositories",
        title: copy::STEP_REPOS,
        subtitle: org_label.as_deref(),
        buttons: &[copy::DONE],
        danger: false,
        width: 560.0,
    };
    let Some(org) = st.draft.orgs.get_mut(st.org) else {
        return ModalOutcome::Dismissed;
    };
    let id = org.org_id().cloned();
    let state = cx.state;
    let catalog = CatalogState::of(id.as_ref().and_then(|id| state.org(id)));
    let filter = &mut st.scope_filter;
    let mut reload = false;
    let outcome = modal::modal(ctx, &theme, &spec, |ui| {
        reload = scope_picker(ui, &theme, filter, catalog, &mut org.scope);
    });
    if reload && let Some(id) = id {
        cx.push(UiAction::Send(Command::LoadCatalog {
            org: id,
            force: true,
        }));
    }
    outcome
}

fn people_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    editor: Editor,
) -> ModalOutcome {
    let theme = cx.theme.clone();
    let saved = saved(st);
    let Some(org) = st.draft.orgs.get_mut(st.org) else {
        return ModalOutcome::Dismissed;
    };
    let id = saved.then(|| org.org_id().cloned()).flatten();
    let (title, picker) = match editor {
        Editor::Reviewers => (copy::REVIEWERS_COUNT, &mut st.pickers.reviewers),
        Editor::Ignored => (copy::IGNORE_FROM, &mut st.pickers.ignored),
        Editor::Tracked | Editor::AddOrg | Editor::Licenses | Editor::Repositories => {
            (copy::TRACK_FROM, &mut st.pickers.tracked)
        }
    };
    let org_label = org.display_label().to_string();
    let spec = ModalSpec {
        id: "edit_people",
        title,
        subtitle: Some(&org_label),
        buttons: &[copy::DONE],
        danger: false,
        width: 480.0,
    };
    let list = people_list(org, editor);
    let outcome = modal::modal(ctx, &theme, &spec, |ui| {
        people_picker(ui, cx, picker, id.as_ref(), list);
    });
    if outcome != ModalOutcome::Open {
        picker.close();
    }
    outcome
}
