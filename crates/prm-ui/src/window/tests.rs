use eframe::egui::{RawInput, ViewportId, ViewportInfo, pos2, vec2};
use prm_config::UiPrefs;
use prm_domain::testing::ts;
use prm_service::AppState;

use super::*;
use crate::testing::link;

fn launch(hide_offscreen: bool, restore_position: Option<Pos2>) -> WindowLaunch {
    WindowLaunch {
        start_hidden: true,
        hide_offscreen,
        restore_position,
    }
}

/// Runs one pass with `info` as the root viewport and returns the viewport commands sent.
fn pass(
    ctx: &egui::Context,
    info: ViewportInfo,
    run: impl FnOnce(&egui::Context),
) -> Vec<ViewportCommand> {
    let mut input = RawInput::default();
    input.viewports.insert(ViewportId::ROOT, info);
    ctx.begin_pass(input);
    run(ctx);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    output
        .viewport_output
        .remove(&ViewportId::ROOT)
        .map(|v| v.commands)
        .unwrap_or_default()
}

/// A show as the app does it: requested, then sent from the same frame's logic.
fn show_now(window: &mut WindowController, ctx: &egui::Context) {
    window.show(ctx);
    window.take_show(ctx);
}

fn moves(commands: &[ViewportCommand]) -> Vec<Pos2> {
    commands
        .iter()
        .filter_map(|c| {
            if let ViewportCommand::OuterPosition(at) = c {
                Some(*at)
            } else {
                None
            }
        })
        .collect()
}

/// On Linux and macOS a start-hidden window waited at -32000,-32000 and "Open PR Monitor"
/// showed it there, where nobody could see it.
#[test]
fn window_offscreen_start_moves_back_to_the_saved_position_on_first_show() {
    let ctx = egui::Context::default();
    ctx.set_zoom_factor(2.0);
    let (link, _ends) = link(AppState::default());
    let saved = pos2(300.0, 200.0);
    let mut window = WindowController::new(launch(true, Some(saved)), &UiPrefs::default());
    pass(&ctx, ViewportInfo::default(), |ctx| {
        window.begin_frame(ctx, &link, ts(0), true);
    });
    assert!(!window.visible());
    assert!(
        !window.take_uncloak(),
        "an off-screen window is not cloaked"
    );
    let commands = pass(&ctx, ViewportInfo::default(), |ctx| {
        show_now(&mut window, ctx)
    });
    assert_eq!(moves(&commands), [pos2(150.0, 100.0)], "zoomed points");
    let again = pass(&ctx, ViewportInfo::default(), |ctx| {
        show_now(&mut window, ctx)
    });
    assert_eq!(moves(&again), [], "only the first show moves it");
}

#[test]
fn window_offscreen_start_without_a_saved_position_centers_on_the_monitor() {
    let ctx = egui::Context::default();
    let (link, _ends) = link(AppState::default());
    let mut window = WindowController::new(launch(true, None), &UiPrefs::default());
    let info = ViewportInfo {
        monitor_size: Some(vec2(1920.0, 1080.0)),
        outer_rect: Some(Rect::from_min_size(OFFSCREEN, vec2(1280.0, 760.0))),
        ..ViewportInfo::default()
    };
    pass(&ctx, info.clone(), |ctx| {
        window.begin_frame(ctx, &link, ts(0), true);
    });
    let commands = pass(&ctx, info, |ctx| show_now(&mut window, ctx));
    assert_eq!(moves(&commands), [pos2(320.0, 160.0)]);
}

/// A cloaked start-hidden window stays cloaked while hidden, and its first show asks for the
/// cloak again so nothing is composed before a themed frame is presented.
#[test]
fn window_cloaked_start_stays_cloaked_until_shown_and_never_moves() {
    let ctx = egui::Context::default();
    let (link, _ends) = link(AppState::default());
    let mut window = WindowController::new(launch(false, None), &UiPrefs::default());
    pass(&ctx, ViewportInfo::default(), |ctx| {
        window.begin_frame(ctx, &link, ts(0), true);
    });
    assert!(!window.take_uncloak(), "hidden windows stay cloaked");
    let mut cloak = false;
    let commands = pass(&ctx, ViewportInfo::default(), |ctx| {
        window.show(ctx);
        cloak = window.take_show(ctx);
    });
    assert!(cloak, "showing from hidden re-cloaks first");
    assert_eq!(moves(&commands), []);
}

/// The saved rect was the outer rect in zoomed points but was restored as the inner size in
/// unzoomed points, so the window grew (or shrank) on every launch.
#[test]
fn window_placement_is_outer_position_and_inner_size_unzoomed() {
    let outer = Rect::from_min_size(pos2(100.0, 50.0), vec2(816.0, 539.0));
    let inner = Rect::from_min_size(pos2(108.0, 81.0), vec2(800.0, 500.0));
    let placement = placement_of(outer, inner, 1.5).expect("on screen");
    assert_eq!(placement.min, pos2(150.0, 75.0));
    assert_eq!(placement.size(), vec2(1200.0, 750.0));
    let parked = Rect::from_min_size((OFFSCREEN.to_vec2() / 1.5).to_pos2(), outer.size());
    assert_eq!(placement_of(parked, inner, 1.5), None);
}
