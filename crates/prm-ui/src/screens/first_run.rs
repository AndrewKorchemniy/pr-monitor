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
    egui::CentralPanel::no_frame()
        .frame(adapter::frame_canvas(&theme))
        .show(ui, |ui| {
            card::anchored_card(ui, &theme, "first_run", 560.0, |ui| {
                header(ui, &theme, fr.step);
                match fr.step {
                    Step::Connect => connect(ui, cx, fr),
                    Step::Repositories => repositories(ui, cx, fr),
                    Step::People => people(ui, cx, fr),
                }
            });
        });
}

/// "PR Monitor", then "Step 2 of 3 · Repositories".
fn header(ui: &mut egui::Ui, theme: &UiTheme, step: Step) {
    let (index, name) = match step {
        Step::Connect => (1, copy::CONNECT),
        Step::Repositories => (2, copy::REPOSITORIES),
        Step::People => (3, copy::PEOPLE),
    };
    ui.label(
        RichText::new(copy::APP_NAME)
            .font(theme.fonts.display.clone())
            .color(theme.colors.text_primary),
    );
    let caption = format!("{} · {name}", copy::step_of(index, 3));
    widgets::muted(ui, theme, &caption);
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
        tall: true,
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
    if let Some(identity) = &identity {
        widgets::muted(ui, &theme, identity);
        ui.add_space(theme.tokens.metrics.space.md);
    }
    let filter = &mut st.scope_filter;
    let reload = scope_picker(ui, &theme, filter, catalog.as_deref(), &mut org.scope);
    ui.add_space(theme.tokens.metrics.space.lg);
    if reload && let Some(id) = org.org_id() {
        cx.push(UiAction::Send(prm_service::Command::LoadCatalog {
            org: id.clone(),
            force: true,
        }));
    }
    let mut next = false;
    let back = footer(ui, &theme, |ui| {
        next = widgets::primary_button(ui, &theme, copy::CONTINUE).clicked();
    });
    if back {
        fr.step = Step::Connect;
    } else if next {
        fr.step = Step::People;
    }
}

fn people(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    let st = &mut fr.settings;
    let Some(org) = st.draft.orgs.first_mut() else {
        fr.step = Step::Connect;
        return;
    };
    let id = org.org_id().cloned();
    group_label(ui, &theme, copy::TRACK_FROM, None);
    let list = &mut org.people.tracked_authors;
    people_picker(ui, cx, &mut st.pickers.tracked, id.as_ref(), list);
    ui.add_space(theme.tokens.metrics.space.xl);
    group_label(
        ui,
        &theme,
        copy::COUNT_REVIEWS_FROM,
        Some(copy::REVIEWERS_EMPTY_HELP),
    );
    let list = &mut org.people.reviewers;
    people_picker(ui, cx, &mut st.pickers.reviewers, id.as_ref(), list);
    ui.add_space(theme.tokens.metrics.space.lg);
    // why: once people are chosen, skipping would read as throwing the choice away.
    let nobody = org.people.tracked_authors.is_empty() && org.people.reviewers.is_empty();
    let mut finish = None;
    let back = footer(ui, &theme, |ui| {
        if widgets::primary_button(ui, &theme, copy::START_MONITORING).clicked() {
            finish = Some(true);
        }
        if nobody && ui.button(copy::SKIP_FOR_NOW).clicked() {
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

/// A bold group title with an optional muted line under it.
fn group_label(ui: &mut egui::Ui, theme: &UiTheme, title: &str, help: Option<&str>) {
    ui.label(
        RichText::new(title)
            .font(theme.fonts.body_strong.clone())
            .color(theme.colors.text_primary),
    );
    if let Some(help) = help {
        widgets::muted(ui, theme, help);
    }
    ui.add_space(theme.tokens.metrics.space.xs);
}

/// A Back button on the left, the step's buttons right-aligned; true on Back.
fn footer(ui: &mut egui::Ui, theme: &UiTheme, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut went_back = false;
    ui.add_space(theme.tokens.metrics.space.xs);
    ui.horizontal(|ui| {
        went_back = ui.button(copy::PREVIOUS).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
    went_back
}
