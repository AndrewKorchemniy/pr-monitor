//! What widgets ask the app to do. Widgets never mutate app state directly; they push actions
//! that the app runs after drawing.

use std::sync::Arc;

use prm_domain::{
    OrgId, PrKey,
    auth::{SettingsSection, UserAction},
    view::StatusFilter,
};
use prm_service::Command;

use crate::{commands::CommandId, route::ModalKind};

#[derive(Debug)]
pub(crate) enum UiAction {
    Run(CommandId),
    Send(Command),
    /// A suggested fix from a banner or error, for an org when it names one.
    User {
        org: Option<OrgId>,
        action: UserAction,
    },
    OpenSettings(SettingsSection),
    Modal(ModalKind),
    Select(PrKey),
    Open(PrKey),
    SetStatus(StatusFilter),
    ClearFilters,
    SaveSettings,
    /// First run is done (finished or skipped).
    FinishFirstRun,
    /// Leave settings (asks first when there are unsaved changes).
    LeaveSettings,
    DiscardSettings,
    ReloadThemes,
    /// Settings picked a theme to preview live; None reverts.
    PreviewTheme(Option<String>),
    SetFilterText(String),
    /// Puts the text on the clipboard.
    Copy(String),
    DismissBanner(Arc<str>),
    /// The table's header clicked a sortable column.
    Sort(prm_domain::view::SortKey),
    ToggleGroup(u32),
    /// The column menu or the settings checklist showed or hid a column.
    ToggleColumn(crate::view::columns::ColumnId),
}

/// Collected during a frame, run afterwards.
#[derive(Debug, Default)]
pub(crate) struct Actions(Vec<UiAction>);

impl Actions {
    pub(crate) fn push(&mut self, action: UiAction) {
        self.0.push(action);
    }

    pub(crate) fn take(&mut self) -> Vec<UiAction> {
        std::mem::take(&mut self.0)
    }
}
