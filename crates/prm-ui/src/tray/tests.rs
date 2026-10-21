use prm_service::AuthStateView;

use super::*;
use crate::testing::org_status;

fn colors(badge: TrayBadge) -> MarkColors {
    let palette = &crate::theme::fallback().tokens.palette;
    MarkColors::from_palette(palette, bar_color(palette, badge))
}

#[test]
fn tray_shown_first_sync_sets_the_icon_and_tooltip() {
    let mut shown = TrayShown::default();
    let state = Arc::new(AppState::default());
    let update = shown.update(&state, colors(TrayBadge::Idle));
    assert_eq!(update.icon, Some(colors(TrayBadge::Idle)));
    assert_eq!(update.tooltip, Some(state.tray_tooltip()));
    assert_eq!(update.paused, None, "the pause item starts as pause");
    assert_eq!(update.sign_in, None, "the sign-in item starts disabled");
}

#[test]
fn tray_shown_same_snapshot_and_colors_changes_nothing() {
    let mut shown = TrayShown::default();
    let state = Arc::new(AppState::default());
    shown.update(&state, colors(TrayBadge::Idle));
    assert_eq!(
        shown.update(&state, colors(TrayBadge::Idle)),
        TrayUpdate::default()
    );
}

/// The OS switching light/dark changes the mark colors with no new snapshot and no theme
/// rebuild; the tray used to keep the old palette until the badge changed.
#[test]
fn tray_shown_color_change_alone_redraws_the_icon() {
    let mut shown = TrayShown::default();
    let state = Arc::new(AppState::default());
    shown.update(&state, colors(TrayBadge::Idle));
    let update = shown.update(&state, colors(TrayBadge::Attention));
    assert_eq!(
        update,
        TrayUpdate {
            icon: Some(colors(TrayBadge::Attention)),
            ..TrayUpdate::default()
        }
    );
}

#[test]
fn tray_shown_sign_in_item_follows_orgs_that_need_sign_in() {
    let mut shown = TrayShown::default();
    shown.update(&Arc::new(AppState::default()), colors(TrayBadge::Idle));
    let signed_out = Arc::new(AppState {
        orgs: Arc::from([org_status("contoso", AuthStateView::SignedOut)]),
        ..AppState::default()
    });
    assert_eq!(
        shown.update(&signed_out, colors(TrayBadge::Idle)).sign_in,
        Some(true)
    );
    let checking = Arc::new(AppState {
        orgs: Arc::from([org_status("contoso", AuthStateView::Checking)]),
        ..AppState::default()
    });
    assert_eq!(
        shown.update(&checking, colors(TrayBadge::Idle)).sign_in,
        Some(false)
    );
}

#[test]
fn tray_labels_match_their_command_titles() {
    let cases = [
        (copy::TRAY_OPEN, CommandId::WindowShow),
        (copy::TRAY_REFRESH, CommandId::RefreshNow),
        (copy::TRAY_PAUSE, CommandId::NotificationsPause),
        (copy::TRAY_RESUME, CommandId::NotificationsResume),
        (copy::TRAY_SIGN_IN, CommandId::AuthMenu),
        (copy::TRAY_QUIT, CommandId::AppQuit),
    ];
    for (label, command) in cases {
        assert_eq!(label, command.title(), "{command:?}");
    }
}

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
