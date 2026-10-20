//! Organizations: the list, adding one by URL, and for the selected org its connection,
//! repositories and people.

mod sign_in;

use eframe::egui::{self, RichText};
use prm_config::OrgProfile;
use prm_domain::{OrgId, classify::EmptyReviewerPolicy, ids::parse_org_input, plan::DraftPolicy};
use prm_service::Command;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    route::ModalKind,
    screens::settings::SettingsState,
    theme::UiTheme,
    widgets::{self, people_picker::people_picker, scope_picker::scope_picker},
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
    org_list(ui, cx, st);
    ui.add_space(theme.tokens.metrics.space.xl);
    if st.org >= st.draft.orgs.len() {
        return;
    }
    let saved = st.draft.orgs[st.org]
        .org_id()
        .is_some_and(|id| st.base.org(id).is_some());
    let space = theme.tokens.metrics.space.xl;
    details(ui, &theme, &mut st.draft.orgs[st.org]);
    ui.add_space(space);
    widgets::heading(ui, &theme, copy::SIGN_IN_METHOD);
    sign_in::section(ui, cx, st, saved);
    ui.add_space(space);
    widgets::heading(ui, &theme, copy::STEP_REPOS);
    repositories(ui, cx, st);
    ui.add_space(space);
    widgets::heading(ui, &theme, copy::STEP_PEOPLE);
    people(ui, cx, st, saved);
    ui.add_space(space);
    if ui.button(copy::REMOVE_ORG).clicked() {
        cx.push(UiAction::Modal(ModalKind::RemoveOrg(st.org)));
    }
}

fn org_list(ui: &mut egui::Ui, cx: &FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    ui.horizontal_wrapped(|ui| {
        for (index, org) in st.draft.orgs.iter().enumerate() {
            let label = RichText::new(org.display_label().as_ref()).font(theme.fonts.body.clone());
            if ui
                .add(egui::Button::selectable(st.org == index, label))
                .clicked()
                && st.org != index
            {
                st.org = index;
                st.pickers = super::Pickers::default();
            }
        }
    });
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut st.new_org_url)
            .hint_text(copy::ORG_URL_HINT)
            .desired_width(320.0);
        let response = ui.add(edit);
        let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button(copy::ADD_ORG).clicked() || submitted) && !st.new_org_url.trim().is_empty() {
            add_org(st);
        }
    });
    if st.url_error {
        let danger = theme.colors.tone(crate::theme::Tone::Danger).fg;
        ui.label(RichText::new(copy::BAD_ORG_URL).color(danger));
    }
}

fn add_org(st: &mut SettingsState) {
    match new_profile(&st.new_org_url) {
        Some(profile) => {
            let existing = st
                .draft
                .orgs
                .iter()
                .position(|o| o.org_id() == profile.id.as_ref());
            st.org = existing.unwrap_or_else(|| {
                st.draft.orgs.push(profile);
                st.draft.orgs.len() - 1
            });
            st.new_org_url.clear();
            st.url_error = false;
            st.pickers = super::Pickers::default();
        }
        None => st.url_error = true,
    }
}

fn details(ui: &mut egui::Ui, theme: &UiTheme, org: &mut OrgProfile) {
    egui::Grid::new("org_details")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            widgets::field_label(ui, theme, copy::NAME);
            ui.add(egui::TextEdit::singleline(&mut org.label).desired_width(320.0));
            ui.end_row();
            widgets::field_label(ui, theme, copy::URL);
            ui.label(RichText::new(&org.url).font(theme.fonts.mono.clone()));
            ui.end_row();
        });
    ui.checkbox(&mut org.enabled, copy::WATCH_ORG);
    ui.checkbox(&mut org.muted, copy::MUTE_ORG);
}

fn repositories(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    let org = &mut st.draft.orgs[st.org];
    let id: Option<OrgId> = org.org_id().cloned();
    let catalog = id
        .as_ref()
        .and_then(|id| cx.state.org(id))
        .and_then(|o| o.catalog.clone());
    if catalog.is_none()
        && let Some(id) = &id
    {
        request_catalog(ui, cx, id, false);
    }
    let reload = scope_picker(
        ui,
        &theme,
        &mut st.scope_filter,
        catalog.as_deref(),
        &mut org.scope,
    );
    if reload && let Some(id) = &id {
        cx.push(UiAction::Send(Command::LoadCatalog {
            org: id.clone(),
            force: true,
        }));
    }
    let mut drafts = org.drafts == DraftPolicy::Show;
    if ui.checkbox(&mut drafts, copy::INCLUDE_DRAFTS).changed() {
        org.drafts = if drafts {
            DraftPolicy::Show
        } else {
            DraftPolicy::Hide
        };
    }
    ui.checkbox(&mut org.org_wide_query, copy::ORG_WIDE_QUERY);
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

fn people(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState, saved: bool) {
    let theme = cx.theme.clone();
    let org = &mut st.draft.orgs[st.org];
    let id = saved.then(|| org.org_id().cloned()).flatten();
    widgets::field_label(ui, &theme, copy::TRACK_FROM);
    people_picker(
        ui,
        cx,
        &mut st.pickers.tracked,
        id.as_ref(),
        &mut org.people.tracked_authors,
    );
    ui.checkbox(&mut org.include_me, copy::INCLUDE_ME);
    ui.add_space(theme.tokens.metrics.space.lg);
    widgets::field_label(ui, &theme, copy::REVIEWERS_COUNT);
    widgets::muted(ui, &theme, copy::REVIEWERS_HELP);
    people_picker(
        ui,
        cx,
        &mut st.pickers.reviewers,
        id.as_ref(),
        &mut org.people.reviewers,
    );
    if org.people.reviewers.is_empty() {
        widgets::field_label(ui, &theme, copy::EMPTY_LIST_POLICY);
        let policy = &mut org.review.empty_reviewer_list;
        ui.radio_value(
            policy,
            EmptyReviewerPolicy::AnyNonAuthorVote,
            copy::ANY_NON_AUTHOR,
        );
        ui.radio_value(policy, EmptyReviewerPolicy::Nobody, copy::NO_VOTES);
    }
    ui.add_space(theme.tokens.metrics.space.lg);
    widgets::field_label(ui, &theme, copy::IGNORE_FROM);
    widgets::muted(ui, &theme, copy::IGNORE_HELP);
    people_picker(
        ui,
        cx,
        &mut st.pickers.ignored,
        id.as_ref(),
        &mut org.people.ignored_comment_authors,
    );
}
