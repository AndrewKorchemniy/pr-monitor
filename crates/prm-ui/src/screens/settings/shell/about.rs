//! The version and the links to the licenses, the source and problem reports: at the foot of the
//! sidebar, or as the last section of General once the sidebar folds into the page menu.

use eframe::egui::{self, RichText};
use prm_domain::app::{DocLink, REPO_URL};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::{Editor, SettingsEnv, SettingsState},
    theme::UiTheme,
    widgets,
};

/// Where an about link goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Link {
    Url(&'static str),
    Licenses,
}

/// The links, in reading order.
const LINKS: [(&str, Link); 3] = [
    (copy::LICENSES_SHORT, Link::Licenses),
    (copy::SOURCE_CODE, Link::Url(REPO_URL)),
    (
        copy::REPORT_PROBLEM,
        Link::Url(DocLink::ReportProblem.url()),
    ),
];

/// The foot of the sidebar, in its bottom-up layout: the links, with the version above them.
pub(super) fn footer(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let mut opened = None;
    // why: bottom-up layout, so the last link added sits highest.
    for (label, link) in LINKS.into_iter().rev() {
        if ui.link(small(&theme, label)).clicked() {
            opened = Some(link);
        }
    }
    ui.add_space(theme.tokens.metrics.space.xs);
    version(ui, &theme, env);
    if let Some(link) = opened {
        follow(cx, st, link);
    }
}

/// The version, then the links on lines that wrap.
pub(super) fn section(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    version(ui, &theme, env);
    let mut opened = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.lg;
        for (label, link) in LINKS {
            if ui.link(small(&theme, label)).clicked() {
                opened = Some(link);
            }
        }
    });
    if let Some(link) = opened {
        follow(cx, st, link);
    }
}

fn small(theme: &UiTheme, text: &str) -> RichText {
    RichText::new(text).font(theme.fonts.small.clone())
}

/// "PR Monitor 0.2.0", with the commit and target on hover.
fn version(ui: &mut egui::Ui, theme: &UiTheme, env: &SettingsEnv<'_>) {
    let build = format!("{} · {}", env.build.git_sha, env.build.target);
    let version = format!("{} {}", copy::APP_NAME, env.build.version);
    widgets::muted(ui, theme, &version).on_hover_text(build);
}

fn follow(cx: &mut FrameCx<'_>, st: &mut SettingsState, link: Link) {
    match link {
        Link::Url(url) => {
            if let Ok(url) = url::Url::parse(url) {
                cx.push(UiAction::Send(prm_service::Command::OpenUrl(url)));
            }
        }
        Link::Licenses => st.editing = Some(Editor::Licenses),
    }
}
