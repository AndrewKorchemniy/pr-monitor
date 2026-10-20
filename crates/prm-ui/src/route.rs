//! Where the user is (screen, overlay, modal) and what Esc does, as a pure decision.

use prm_domain::OrgId;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Screen {
    #[default]
    Dashboard,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Overlay {
    #[default]
    None,
    Palette,
    Shortcuts,
    Modal(ModalKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModalKind {
    DiscardSettings,
    RemoveOrg(usize),
    Pat(OrgId),
    ContactAdmin(String),
    AuthMenu,
}

/// What Esc should do; the first match wins.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum EscapeTarget {
    /// An egui popup is open: egui closes it.
    Popup,
    CloseOverlay,
    ClearFilterText,
    BlurFilter,
    ClosePeek,
    LeaveSettings,
    ClearFilters,
    HideToTray,
    Nothing,
}

/// The facts Esc depends on.
#[derive(Clone, Debug, Default)]
pub(crate) struct EscapeInputs {
    pub popup_open: bool,
    pub overlay: Overlay,
    pub filter_focused: bool,
    pub filter_empty: bool,
    pub peek_open: bool,
    pub screen: Screen,
    pub filters_active: bool,
    pub has_tray: bool,
}

pub(crate) fn escape_target(input: &EscapeInputs) -> EscapeTarget {
    if input.popup_open {
        EscapeTarget::Popup
    } else if input.overlay != Overlay::None {
        EscapeTarget::CloseOverlay
    } else if input.filter_focused && !input.filter_empty {
        EscapeTarget::ClearFilterText
    } else if input.filter_focused {
        EscapeTarget::BlurFilter
    } else if input.peek_open && input.screen == Screen::Dashboard {
        EscapeTarget::ClosePeek
    } else if input.screen == Screen::Settings {
        EscapeTarget::LeaveSettings
    } else if input.filters_active {
        EscapeTarget::ClearFilters
    } else if input.has_tray {
        EscapeTarget::HideToTray
    } else {
        EscapeTarget::Nothing
    }
}

#[cfg(test)]
mod tests;
