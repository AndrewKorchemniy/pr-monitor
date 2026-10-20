use super::*;

#[test]
fn the_bar_color_follows_the_badge() {
    let palette = &crate::theme::fallback().tokens.palette;
    assert_eq!(bar_color(palette, TrayBadge::Idle), palette.tray_idle);
    assert_eq!(
        bar_color(palette, TrayBadge::Attention),
        palette.status_needs_review_solid
    );
    assert_eq!(
        bar_color(palette, TrayBadge::Error),
        palette.feedback_danger_solid
    );
    assert_eq!(bar_color(palette, TrayBadge::Syncing), palette.accent_base);
}

#[test]
fn the_window_icon_is_a_square_rgba_raster() {
    let icon = window_icon();
    assert_eq!(
        (icon.width, icon.height),
        (WINDOW_ICON_SIZE, WINDOW_ICON_SIZE)
    );
    assert_eq!(
        icon.rgba.len(),
        (WINDOW_ICON_SIZE * WINDOW_ICON_SIZE * 4) as usize
    );
    assert!(
        icon.rgba.chunks(4).any(|px| px[3] == 255),
        "the mark is opaque somewhere"
    );
}

#[test]
fn menu_ids_are_command_keys() {
    for command in [
        CommandId::WindowShow,
        CommandId::RefreshNow,
        CommandId::NotificationsPause,
        CommandId::AuthMenu,
        CommandId::SettingsOpen,
        CommandId::AppQuit,
    ] {
        assert_eq!(CommandId::from_key(command.key()), Some(command));
    }
}
