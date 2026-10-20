use super::*;

fn dashboard() -> EscapeInputs {
    EscapeInputs {
        has_tray: true,
        ..EscapeInputs::default()
    }
}

#[test]
fn escape_closes_the_innermost_thing_first() {
    let all = EscapeInputs {
        popup_open: true,
        overlay: Overlay::Palette,
        filter_focused: true,
        peek_open: true,
        filters_active: true,
        ..dashboard()
    };
    assert_eq!(escape_target(&all), EscapeTarget::Popup);
    let no_popup = EscapeInputs {
        popup_open: false,
        ..all.clone()
    };
    assert_eq!(escape_target(&no_popup), EscapeTarget::CloseOverlay);
    let no_overlay = EscapeInputs {
        overlay: Overlay::None,
        ..no_popup
    };
    assert_eq!(escape_target(&no_overlay), EscapeTarget::ClearFilterText);
    let empty_filter = EscapeInputs {
        filter_empty: true,
        ..no_overlay.clone()
    };
    assert_eq!(escape_target(&empty_filter), EscapeTarget::BlurFilter);
    let blurred = EscapeInputs {
        filter_focused: false,
        ..no_overlay
    };
    assert_eq!(escape_target(&blurred), EscapeTarget::ClosePeek);
    let no_peek = EscapeInputs {
        peek_open: false,
        ..blurred
    };
    assert_eq!(escape_target(&no_peek), EscapeTarget::ClearFilters);
    let no_filters = EscapeInputs {
        filters_active: false,
        ..no_peek
    };
    assert_eq!(escape_target(&no_filters), EscapeTarget::HideToTray);
}

#[test]
fn escape_leaves_settings_before_touching_dashboard_state() {
    let input = EscapeInputs {
        screen: Screen::Settings,
        peek_open: true,
        filters_active: true,
        ..dashboard()
    };
    assert_eq!(escape_target(&input), EscapeTarget::LeaveSettings);
}

#[test]
fn escape_does_nothing_without_a_tray() {
    let input = EscapeInputs {
        has_tray: false,
        ..dashboard()
    };
    assert_eq!(escape_target(&input), EscapeTarget::Nothing);
}
