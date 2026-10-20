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
    theme::{UiTheme, adapter, icons::Icon},
    widgets::{
        self, card,
        controls::{self, ActionField},
        people_picker::people_picker,
        scope_picker::scope_picker,
    },
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
    let width = if fr.step == Step::Connect {
        460.0
    } else {
        620.0
    };
    egui::CentralPanel::no_frame()
        .frame(adapter::frame_canvas(&theme))
        .show(ui, |ui| {
            card::centered_card(ui, &theme, "first_run", width, |ui| {
                header(ui, &theme, fr.step);
                match fr.step {
                    Step::Connect => connect(ui, cx, fr),
                    Step::Repositories => repositories(ui, cx, fr),
                    Step::People => people(ui, cx, fr),
                }
            });
        });
}

/// "PR Monitor" with the step dots on the right.
fn header(ui: &mut egui::Ui, theme: &UiTheme, step: Step) {
    let index = match step {
        Step::Connect => 0,
        Step::Repositories => 1,
        Step::People => 2,
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(copy::APP_NAME)
                .font(theme.fonts.display.clone())
                .color(theme.colors.text_primary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            card::step_dots(ui, theme, index, 3);
        });
    });
    ui.add_space(theme.tokens.metrics.space.xl);
}

fn connect(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    let st = &mut fr.settings;
    widgets::field_label(ui, &theme, copy::ORG_URL_LABEL);
    ui.add_space(theme.tokens.metrics.space.xs);
    let field = ActionField {
        id: egui::Id::unique("first_run_org_url"),
        hint: copy::ORG_URL_HINT,
        action: copy::CONNECT,
        icon: Icon::ArrowRight,
        error: st.url_error,
    };
    let submitted = controls::action_field(ui, &theme, &mut st.new_org_url, &field);
    if st.url_error {
        ui.add_space(theme.tokens.metrics.space.xs);
        widgets::error_text(ui, &theme, copy::BAD_ORG_URL);
    }
    if !submitted {
        return;
    }
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

fn repositories(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    let st = &mut fr.settings;
    let Some(org) = st.draft.orgs.first_mut() else {
        fr.step = Step::Connect;
        return;
    };
    let status = org.org_id().and_then(|id| cx.state.org(id));
    let identity = status.map(|s| copy::identity(&s.label, s.auth.method));
    let catalog = status.and_then(|o| o.catalog.clone());
    let reload = card::section(ui, &theme, copy::REPOSITORIES, identity.as_deref(), |ui| {
        scope_picker(
            ui,
            &theme,
            &mut st.scope_filter,
            catalog.as_deref(),
            &mut org.scope,
        )
    });
    if reload && let Some(id) = org.org_id() {
        cx.push(UiAction::Send(prm_service::Command::LoadCatalog {
            org: id.clone(),
            force: true,
        }));
    }
    footer(ui, &theme, false, |ui| {
        if widgets::primary_button(ui, &theme, copy::CONTINUE).clicked() {
            fr.step = Step::People;
        }
    });
}

fn people(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    let st = &mut fr.settings;
    let Some(org) = st.draft.orgs.first_mut() else {
        fr.step = Step::Connect;
        return;
    };
    let id = org.org_id().cloned();
    card::section(ui, &theme, copy::TRACK_FROM, None, |ui| {
        let list = &mut org.people.tracked_authors;
        people_picker(ui, cx, &mut st.pickers.tracked, id.as_ref(), list);
    });
    let help = Some(copy::REVIEWERS_EMPTY_HELP);
    card::section(ui, &theme, copy::COUNT_REVIEWS_FROM, help, |ui| {
        let list = &mut org.people.reviewers;
        people_picker(ui, cx, &mut st.pickers.reviewers, id.as_ref(), list);
    });
    let mut finish = None;
    let back = footer(ui, &theme, true, |ui| {
        if widgets::primary_button(ui, &theme, copy::START_MONITORING).clicked() {
            finish = Some(true);
        }
        if ui.button(copy::SKIP_FOR_NOW).clicked() {
            finish = Some(false);
        }
    });
    if back {
        fr.step = Step::Repositories;
    }
    if let Some(refresh) = finish {
        cx.push(UiAction::Send(prm_service::Command::ApplyConfig(Box::new(
            st.draft.clone(),
        ))));
        if refresh {
            cx.push(UiAction::Send(prm_service::Command::RefreshNow));
        }
        cx.push(UiAction::FinishFirstRun);
    }
}

/// A Back button on the left (when `back`), the step's buttons right-aligned; true on Back.
fn footer(ui: &mut egui::Ui, theme: &UiTheme, back: bool, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut went_back = false;
    ui.add_space(theme.tokens.metrics.space.xs);
    ui.horizontal(|ui| {
        if back {
            went_back = ui.button(copy::PREVIOUS).clicked();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
    went_back
}
