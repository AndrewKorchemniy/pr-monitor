//! First run: connect an organization, choose repositories, choose people. Each step edits a
//! draft config that is applied when the user finishes (or skips).

use std::sync::Arc;

use eframe::egui::{self, RichText};
use prm_config::Config;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::{SettingsState, orgs::new_profile},
    widgets::{self, people_picker::people_picker, scope_picker::scope_picker},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Connect,
    Repositories,
    People,
}

pub(crate) struct FirstRun {
    pub step: Step,
    /// Reuses settings' draft, pickers and scope filter.
    pub settings: SettingsState,
}

impl FirstRun {
    pub(crate) fn new(base: Arc<Config>) -> Self {
        Self {
            step: Step::Connect,
            settings: SettingsState::new(base, prm_domain::auth::SettingsSection::Organizations),
        }
    }
}

pub(crate) fn first_run(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    egui::CentralPanel::default_margins().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(560.0);
            ui.add_space(48.0);
            ui.label(RichText::new(copy::FIRST_RUN_HEADLINE).font(theme.fonts.display.clone()));
            ui.add_space(theme.tokens.metrics.space.md);
            ui.label(RichText::new(copy::FIRST_RUN_SUB).color(theme.colors.text_secondary));
            ui.add_space(theme.tokens.metrics.space.xxl);
        });
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match fr.step {
                Step::Connect => connect(ui, cx, fr),
                Step::Repositories => repositories(ui, cx, fr),
                Step::People => people(ui, cx, fr),
            });
    });
}

fn connect(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    widgets::heading(ui, &theme, copy::STEP_CONNECT);
    widgets::field_label(ui, &theme, copy::ORG_URL_LABEL);
    let st = &mut fr.settings;
    ui.add(
        egui::TextEdit::singleline(&mut st.new_org_url)
            .hint_text(copy::ORG_URL_HINT)
            .desired_width(420.0),
    );
    widgets::muted(ui, &theme, copy::ORG_URL_HELP);
    if st.url_error {
        ui.label(
            RichText::new(copy::BAD_ORG_URL)
                .color(theme.colors.tone(crate::theme::Tone::Danger).fg),
        );
    }
    if widgets::primary_button(ui, &theme, copy::CONNECT).clicked() {
        match new_profile(&st.new_org_url) {
            Some(profile) => {
                st.url_error = false;
                st.draft.orgs = vec![profile];
                st.org = 0;
                // why: the service only signs in to orgs it knows, so apply the org right away.
                cx.push(UiAction::Send(prm_service::Command::ApplyConfig(Box::new(
                    st.draft.clone(),
                ))));
                fr.step = Step::Repositories;
            }
            None => st.url_error = true,
        }
    }
}

fn repositories(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    widgets::heading(ui, &theme, copy::STEP_REPOS);
    let st = &mut fr.settings;
    let Some(org) = st.draft.orgs.first_mut() else {
        fr.step = Step::Connect;
        return;
    };
    if let Some(status) = org.org_id().and_then(|id| cx.state.org(id)) {
        widgets::muted(
            ui,
            &theme,
            &copy::identity(&status.label, status.auth.method),
        );
    }
    let catalog = org
        .org_id()
        .and_then(|id| cx.state.org(id))
        .and_then(|o| o.catalog.clone());
    if scope_picker(
        ui,
        &theme,
        &mut st.scope_filter,
        catalog.as_deref(),
        &mut org.scope,
    ) && let Some(id) = org.org_id()
    {
        cx.push(UiAction::Send(prm_service::Command::LoadCatalog {
            org: id.clone(),
            force: true,
        }));
    }
    ui.add_space(theme.tokens.metrics.space.lg);
    if widgets::primary_button(ui, &theme, copy::STEP_PEOPLE).clicked() {
        fr.step = Step::People;
    }
}

fn people(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    widgets::heading(ui, &theme, copy::STEP_PEOPLE);
    let st = &mut fr.settings;
    let Some(org) = st.draft.orgs.first_mut() else {
        fr.step = Step::Connect;
        return;
    };
    let id = org.org_id().cloned();
    widgets::field_label(ui, &theme, copy::TRACK_FROM);
    people_picker(
        ui,
        cx,
        &mut st.pickers.tracked,
        id.as_ref(),
        &mut org.people.tracked_authors,
    );
    ui.add_space(theme.tokens.metrics.space.lg);
    widgets::field_label(ui, &theme, copy::COUNT_REVIEWS_FROM);
    widgets::muted(ui, &theme, copy::REVIEWERS_EMPTY_HELP);
    people_picker(
        ui,
        cx,
        &mut st.pickers.reviewers,
        id.as_ref(),
        &mut org.people.reviewers,
    );
    ui.add_space(theme.tokens.metrics.space.xl);
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, &theme, copy::START_MONITORING).clicked() {
            cx.push(UiAction::Send(prm_service::Command::ApplyConfig(Box::new(
                st.draft.clone(),
            ))));
            cx.push(UiAction::Send(prm_service::Command::RefreshNow));
            cx.push(UiAction::FinishFirstRun);
        }
        if ui.button(copy::SKIP_FOR_NOW).clicked() {
            cx.push(UiAction::Send(prm_service::Command::ApplyConfig(Box::new(
                st.draft.clone(),
            ))));
            cx.push(UiAction::FinishFirstRun);
        }
    });
}
