//! The update line of About: one status sentence and what the user can do about it. Results show
//! here, where the user is looking; nothing pops up.

use std::borrow::Cow;

use eframe::egui;
use prm_domain::update::{DisabledReason, UpdateStage, UpdateStatus};
use prm_service::Command;

use super::Link;
use crate::{copy, frame::FrameCx, theme::UiTheme, widgets};

/// Something the user can click next to the status.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    CheckNow,
    Restart,
    Releases,
}

impl Action {
    /// The command a click sends the service, if it is not a link.
    pub(super) const fn command(self) -> Option<Command> {
        match self {
            Self::CheckNow => Some(Command::CheckForUpdates),
            Self::Restart => Some(Command::ApplyUpdate),
            Self::Releases => None,
        }
    }

    pub(super) const fn link(self) -> Option<Link> {
        match self {
            Self::Releases => Some(Link::Releases),
            Self::CheckNow | Self::Restart => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::CheckNow => copy::CHECK_NOW,
            Self::Restart => copy::RESTART_TO_UPDATE,
            Self::Releases => copy::RELEASES_PAGE,
        }
    }
}

/// What About shows for a status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct View {
    /// Borrowed unless it carries a number, so a frame allocates only while a download runs.
    pub text: Cow<'static, str>,
    pub actions: &'static [Action],
}

const RESTING: &[Action] = &[Action::CheckNow];
const FAILED: &[Action] = &[Action::CheckNow, Action::Releases];
const RESTART: &[Action] = &[Action::Restart];
const NONE: &[Action] = &[];

pub(super) fn describe(status: &UpdateStatus) -> View {
    let (text, actions) = match status {
        UpdateStatus::Idle => (Cow::Borrowed(copy::UPDATE_NOT_CHECKED), RESTING),
        UpdateStatus::Checking => (Cow::Borrowed(copy::UPDATE_CHECKING), NONE),
        UpdateStatus::UpToDate => (Cow::Borrowed(copy::UPDATE_UP_TO_DATE), RESTING),
        UpdateStatus::Downloading(percent) => (copy::update_downloading(*percent).into(), NONE),
        UpdateStatus::Ready(version) => (copy::update_ready(version).into(), RESTART),
        UpdateStatus::Failed(stage) => (Cow::Borrowed(failure(*stage)), FAILED),
        UpdateStatus::Disabled(reason) => (Cow::Borrowed(disabled(*reason)), NONE),
    };
    View { text, actions }
}

const fn failure(stage: UpdateStage) -> &'static str {
    match stage {
        UpdateStage::Check => copy::UPDATE_CHECK_FAILED,
        UpdateStage::Download => copy::UPDATE_DOWNLOAD_FAILED,
        UpdateStage::Apply => copy::UPDATE_APPLY_FAILED,
    }
}

const fn disabled(reason: DisabledReason) -> &'static str {
    match reason {
        DisabledReason::NotInstalled => copy::UPDATE_NOT_INSTALLED,
        DisabledReason::IsolatedHome => copy::UPDATE_ISOLATED_HOME,
    }
}

/// The status, then its actions; the action clicked, if any. In the sidebar's bottom-up layout
/// the parts go in reverse so they still read top to bottom.
pub(super) fn show(ui: &mut egui::Ui, cx: &FrameCx<'_>, bottom_up: bool) -> Option<Action> {
    let theme = &cx.theme;
    let view = describe(&cx.state.update);
    if bottom_up {
        let clicked = actions(ui, theme, &view);
        widgets::muted(ui, theme, &view.text);
        clicked
    } else {
        widgets::muted(ui, theme, &view.text);
        actions(ui, theme, &view)
    }
}

/// The actions on one line that wraps; the one clicked, if any.
fn actions(ui: &mut egui::Ui, theme: &UiTheme, view: &View) -> Option<Action> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.lg;
        for action in view.actions {
            if ui.link(super::small(theme, action.label())).clicked() {
                clicked = Some(*action);
            }
        }
    });
    clicked
}

#[cfg(test)]
mod tests;
