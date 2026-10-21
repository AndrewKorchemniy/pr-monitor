use std::{sync::Arc, time::Duration};

use eframe::egui::{self, RawInput, Rect, ViewportId, ViewportInfo, pos2, vec2};
use prm_config::{UiPrefs, UiState, WindowRect};
use prm_domain::testing::ts;
use prm_service::{AppState, Command};

use super::*;
use crate::{testing::link, window::WindowLaunch};

const SHOWN: WindowLaunch = WindowLaunch {
    start_hidden: false,
    hide_offscreen: false,
    restore_position: None,
};

fn view() -> DashboardView {
    DashboardView::new(&UiState::default(), Arc::from([]), 1, 24.0)
}

fn window() -> WindowController {
    WindowController::new(SHOWN, &UiPrefs::default())
}

const fn layout<'a>(
    view: &'a DashboardView,
    window: &'a WindowController,
    open: bool,
) -> Layout<'a> {
    Layout {
        view,
        window,
        peek: PeekLayout { open, width: None },
    }
}

fn saves(commands: &[Command]) -> usize {
    commands
        .iter()
        .filter(|c| matches!(c, Command::SaveUiState(_)))
        .count()
}

#[test]
fn ui_state_sync_never_saves_an_unchanged_layout() {
    let (link, mut ends) = link(AppState::default());
    let (view, window) = (view(), window());
    let mut sync = UiStateSync::new(UiState::default());
    let base = UiState::default();
    sync.tick(ts(0), &layout(&view, &window, false), &base, &link);
    sync.tick(ts(10), &layout(&view, &window, false), &base, &link);
    assert_eq!(saves(&ends.drain()), 0);
    assert_eq!(sync.deadline(), None);
}

#[test]
fn ui_state_sync_saves_a_change_once_it_has_been_stable_for_a_second() {
    let (link, mut ends) = link(AppState::default());
    let (view, window) = (view(), window());
    let changed = layout(&view, &window, true);
    let base = UiState::default();
    let mut sync = UiStateSync::new(UiState::default());
    sync.tick(ts(100), &changed, &base, &link);
    assert_eq!(sync.deadline(), Some(ts(100).saturating_add(DEBOUNCE)));
    assert_eq!(saves(&ends.drain()), 0);
    let almost = ts(100).saturating_add(Duration::from_millis(999));
    sync.tick(almost, &changed, &base, &link);
    assert_eq!(saves(&ends.drain()), 0);
    sync.tick(ts(101), &changed, &base, &link);
    assert_eq!(saves(&ends.drain()), 1);
    sync.tick(ts(105), &changed, &base, &link);
    assert_eq!(saves(&ends.drain()), 0, "saved states are not resent");
}

#[test]
fn ui_state_sync_flush_saves_immediately_and_only_when_different() {
    let (link, mut ends) = link(AppState::default());
    let (view, window) = (view(), window());
    let base = UiState::default();
    let mut sync = UiStateSync::new(UiState::default());
    sync.flush(&layout(&view, &window, false), &base, &link);
    assert_eq!(saves(&ends.drain()), 0);
    sync.flush(&layout(&view, &window, true), &base, &link);
    assert_eq!(saves(&ends.drain()), 1);
}

/// The window drifted in size each launch: the outer rect in zoomed points was saved and then
/// restored as an inner size in unzoomed points.
#[test]
fn ui_state_window_rect_round_trips_at_a_non_default_zoom() {
    let ctx = egui::Context::default();
    ctx.set_zoom_factor(1.3);
    let (link, _ends) = link(AppState::default());
    let (view, mut window) = (view(), window());
    let mut input = RawInput::default();
    let info = ViewportInfo {
        outer_rect: Some(Rect::from_min_size(pos2(100.0, 50.0), vec2(816.0, 539.0))),
        inner_rect: Some(Rect::from_min_size(pos2(108.0, 81.0), vec2(800.0, 500.0))),
        ..ViewportInfo::default()
    };
    input.viewports.insert(ViewportId::ROOT, info);
    ctx.begin_pass(input);
    window.begin_frame(&ctx, &link, ts(0), true);
    ctx.end_pass().textures_delta.clear();
    let live = layout(&view, &window, false);
    let saved = live.collect(&UiState::default());
    let rect = saved.window.expect("a window rect");
    let close = |a: f32, b: f32| (a - b).abs() < 0.01;
    let expected = [130.0, 65.0, 1040.0, 650.0];
    let actual = [rect.x, rect.y, rect.w, rect.h];
    assert!(
        actual.iter().zip(expected).all(|(a, b)| close(*a, b)),
        "{rect:?}: outer position and inner size, unzoomed"
    );
    assert!(!rect.maximized);
    assert!(!live.differs(&saved), "a saved layout is not dirty");
    let moved = UiState {
        window: Some(WindowRect { x: 0.0, ..rect }),
        ..saved
    };
    assert!(live.differs(&moved));
}
