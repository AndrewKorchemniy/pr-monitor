//! What every widget sees during one frame: the theme, the service state, the time, the keymap
//! (for hints) and the action queue. Built fresh each frame from the app's fields.

use std::sync::Arc;

use prm_domain::Timestamp;
use prm_service::AppState;
use time::UtcOffset;

use crate::{
    actions::{Actions, UiAction},
    commands::{CommandId, keymap::Keymap},
    theme::UiTheme,
    view::changes::ChangeLog,
};

pub(crate) struct FrameCx<'a> {
    pub theme: Arc<UiTheme>,
    pub state: &'a AppState,
    pub now: Timestamp,
    pub offset: UtcOffset,
    pub keymap: &'a Keymap,
    pub changes: &'a ChangeLog,
    pub actions: &'a mut Actions,
}

impl FrameCx<'_> {
    pub(crate) fn push(&mut self, action: UiAction) {
        self.actions.push(action);
    }

    pub(crate) fn run(&mut self, command: CommandId) {
        self.actions.push(UiAction::Run(command));
    }

    /// "Refresh now (F5)": a title with its first key binding.
    pub(crate) fn titled(&self, command: CommandId) -> String {
        match self.keymap.hint(command) {
            Some(chord) => format!("{} ({chord})", command.title()),
            None => command.title().to_owned(),
        }
    }
}
