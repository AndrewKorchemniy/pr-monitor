//! The version, whether it is up to date, and the links to the release notes, the licenses, the
//! source and problem reports: at the foot of the sidebar, or as the last section of General
//! once the sidebar folds into the page menu.

mod update;

use eframe::egui::{self, RichText};
use prm_domain::app::{DocLink, REPO_URL};
use prm_service::Command;

use self::update::Action;

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
    /// The release notes of the running version.
    WhatsNew,
    /// Where releases are listed, for when updating does not work.
    Releases,
    Licenses,
}

/// The links, in reading order.
const LINKS: [(&str, Link); 4] = [
    (copy::WHATS_NEW, Link::WhatsNew),
    (copy::LICENSES_SHORT, Link::Licenses),
    (copy::SOURCE_CODE, Link::Url(REPO_URL)),
    (
        copy::REPORT_PROBLEM,
        Link::Url(DocLink::ReportProblem.url()),
    ),
];

/// The foot of the sidebar, in its bottom-up layout: the links, with the update line and the
/// version above them.
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
    let action = update::show(ui, cx, update::Stack::BottomUp);
    version(ui, &theme, env);
    finish(cx, st, env, opened, action);
}

/// The version, the update line, then the links on lines that wrap.
pub(super) fn section(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    version(ui, &theme, env);
    let action = update::show(ui, cx, update::Stack::TopDown);
    let mut opened = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.lg;
        for (label, link) in LINKS {
            if ui.link(small(&theme, label)).clicked() {
                opened = Some(link);
            }
        }
    });
    finish(cx, st, env, opened, action);
}

/// Runs what was clicked: an update action, or a link.
fn finish(
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
    opened: Option<Link>,
    action: Option<Action>,
) {
    let link = opened.or_else(|| action.and_then(Action::link));
    if let Some(command) = action.and_then(Action::command) {
        cx.push(UiAction::Send(command));
    }
    if let Some(link) = link {
        follow(cx, st, env, link);
    }
}

/// A link's text: small, in the link colour, which the theme's text-colour override would
/// otherwise replace.
fn small(theme: &UiTheme, text: &str) -> RichText {
    RichText::new(text)
        .font(theme.fonts.small.clone())
        .color(theme.colors.text_link)
}

/// "PR Monitor 0.2.0", with the commit and target on hover.
fn version(ui: &mut egui::Ui, theme: &UiTheme, env: &SettingsEnv<'_>) {
    let build = format!("{} · {}", env.build.git_sha, env.build.target);
    let version = format!("{} {}", copy::APP_NAME, env.build.version);
    widgets::muted(ui, theme, &version).on_hover_text(build);
}

fn follow(cx: &mut FrameCx<'_>, st: &mut SettingsState, env: &SettingsEnv<'_>, link: Link) {
    let url = match link {
        Link::Url(url) => url.to_owned(),
        Link::WhatsNew => whats_new_url(env.build.version),
        Link::Releases => releases_url(),
        Link::Licenses => {
            st.editing = Some(Editor::Licenses);
            return;
        }
    };
    if let Ok(url) = url::Url::parse(&url) {
        cx.push(UiAction::Send(Command::OpenUrl(url)));
    }
}

fn releases_url() -> String {
    format!("{REPO_URL}/releases")
}

/// The release notes of `version`; a pre-release or dev version has no tag of its own, so it
/// gets the list of releases.
fn whats_new_url(version: &str) -> String {
    if version.contains('-') {
        releases_url()
    } else {
        format!("{REPO_URL}/releases/tag/v{version}")
    }
}

#[cfg(test)]
mod tests;
