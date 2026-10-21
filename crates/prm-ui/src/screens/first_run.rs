//! First run: connect an organization, choose repositories, choose people. Each step edits a
//! draft config that is applied when the user finishes (or skips).

use std::sync::Arc;

use eframe::egui::{self, RichText};
use prm_config::Config;
use prm_service::state::Banner;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::{SettingsState, orgs::new_profile},
    theme::{UiTheme, adapter, icons::Icon},
    widgets::{
        self,
        banner::banners,
        card,
        dialog::{self, DialogHead},
        fields::{self, Field},
        people_picker::people_picker,
        scope_picker::{CatalogState, scope_picker},
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

/// The current step in a card; `stopped` (the background service stopped) shows above it, as
/// on the dashboard, since setup cannot finish without the service.
pub(crate) fn first_run(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    fr: &mut FirstRun,
    stopped: Option<&Banner>,
) {
    let theme = cx.theme.clone();
    if let Some(stopped) = stopped {
        let mut expanded = false;
        egui::Panel::top("banners")
            .frame(adapter::frame_canvas(&theme).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| banners(ui, cx, &[stopped], &mut expanded));
    }
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

/// "PR Monitor", then "Step 2 of 3 · Repositories", then a divider.
fn header(ui: &mut egui::Ui, theme: &UiTheme, step: Step) {
    let (index, name) = match step {
        Step::Connect => (1, copy::CONNECT),
        Step::Repositories => (2, copy::REPOSITORIES),
        Step::People => (3, copy::PEOPLE),
    };
    let caption = format!("{} · {name}", copy::step_of(index, 3));
    let head = DialogHead {
        title: copy::APP_NAME,
        subtitle: Some(&caption),
        display: true,
        close: false,
    };
    dialog::header(ui, theme, &head);
}

fn connect(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, fr: &mut FirstRun) {
    let theme = cx.theme.clone();
    let st = &mut fr.settings;
    let submitted = dialog::body(ui, &theme, |ui| {
        widgets::field_label(ui, &theme, copy::ORG_URL_LABEL);
        ui.add_space(theme.tokens.metrics.space.xs);
        let mut field = Field::new(egui::Id::unique("first_run_org_url"), copy::ORG_URL_HINT);
        field.error = st.url_error;
        let action = (&field, copy::CONNECT, Icon::ArrowRight);
        let submitted = fields::action_field(ui, &theme, &mut st.new_org_url, action);
        if st.url_error {
            ui.add_space(theme.tokens.metrics.space.xs);
            widgets::error_text(ui, &theme, copy::BAD_ORG_URL);
        }
        submitted
    });
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
    let state = cx.state;
    let status = org.org_id().and_then(|id| state.org(id));
    let identity = status.map(|s| copy::identity(&s.label, s.auth.method));
    let catalog = CatalogState::of(status);
    let reload = dialog::body(ui, &theme, |ui| {
        if let Some(identity) = &identity {
            widgets::muted(ui, &theme, identity);
            ui.add_space(theme.tokens.metrics.space.md);
        }
        let filter = &mut st.scope_filter;
        scope_picker(ui, &theme, filter, catalog, &mut org.scope)
    });
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
    dialog::body(ui, &theme, |ui| {
        group_label(ui, &theme, copy::TRACK_FROM, Some(copy::TRACK_FROM_HELP));
        let list = &mut org.people.tracked_authors;
        people_picker(ui, cx, &mut st.pickers.tracked, id.as_ref(), list);
        ui.add_space(theme.tokens.metrics.space.xl);
        let help = Some(copy::REVIEWERS_EMPTY_HELP);
        group_label(ui, &theme, copy::COUNT_REVIEWS_FROM, help);
        let list = &mut org.people.reviewers;
        people_picker(ui, cx, &mut st.pickers.reviewers, id.as_ref(), list);
    });
    // why: no "skip": empty lists are a complete choice (everyone, any vote but the
    // author's), so skipping would start the same monitoring.
    let mut start = false;
    let back = footer(ui, &theme, |ui| {
        start = widgets::primary_button(ui, &theme, copy::START_MONITORING).clicked();
    });
    if back {
        fr.step = Step::Repositories;
    }
    if start {
        cx.push(UiAction::Send(prm_service::Command::ApplyConfig(Box::new(
            st.draft.clone(),
        ))));
        cx.push(UiAction::Send(prm_service::Command::RefreshNow));
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
    ui.add_space(theme.tokens.metrics.space.sm);
}

/// The action bar: Back on the left, the step's buttons right-aligned; true on Back.
fn footer(ui: &mut egui::Ui, theme: &UiTheme, add: impl FnOnce(&mut egui::Ui)) -> bool {
    dialog::actions(ui, theme, |ui| {
        let went_back = ui.button(copy::PREVIOUS).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
        went_back
    })
}
