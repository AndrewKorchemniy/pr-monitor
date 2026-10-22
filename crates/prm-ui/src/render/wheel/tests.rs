use eframe::egui::{Event, Modifiers, pos2, vec2};
use egui_kittest::Harness;

use super::*;

/// egui's points per line on native.
const LINE: f32 = 40.0;
/// One frame at 60 Hz, in seconds.
const FRAME: f32 = 1.0 / 60.0;

fn wheel(lines: f32, phase: TouchPhase, source: MouseWheelSource) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, lines),
        phase,
        source,
        modifiers: Modifiers::NONE,
    }
}

fn precise(lines: f32) -> Event {
    wheel(lines, TouchPhase::Move, MouseWheelSource::Unknown)
}

fn start() -> Event {
    wheel(0.0, TouchPhase::Start, MouseWheelSource::Unknown)
}

/// How far egui scrolls in each frame, one frame per entry of `frames`, after `prepare` ran on
/// its events: the `smooth_scroll_delta` every scroll area applies.
fn scrolled_per_frame(frames: Vec<Vec<Event>>, prepare: fn(&mut Vec<Event>)) -> Vec<f32> {
    let mut harness = Harness::builder().with_step_dt(FRAME).build_ui_state(
        |ui, scrolled: &mut f32| *scrolled = -ui.input(|i| i.smooth_scroll_delta().y),
        0.0,
    );
    harness.step();
    let mut scrolled = Vec::new();
    for mut events in frames {
        prepare(&mut events);
        harness.input_mut().events = events;
        harness.step();
        scrolled.push(*harness.state());
    }
    scrolled
}
#[test]
fn mark_precise_scroll_starts_a_gesture_before_the_first_fractional_delta() {
    let mut events = vec![
        Event::PointerMoved(pos2(1.0, 1.0)),
        precise(-0.1),
        precise(-0.2),
    ];
    mark_precise_scroll(&mut events);
    let expected = vec![
        Event::PointerMoved(pos2(1.0, 1.0)),
        start(),
        precise(-0.1),
        precise(-0.2),
    ];
    assert_eq!(events, expected);
}

#[test]
fn mark_precise_scroll_leaves_other_wheel_input_alone() {
    let cases = [
        ("a whole notch", precise(-1.0)),
        ("two notches", precise(2.0)),
        (
            "a macOS trackpad",
            wheel(-0.1, TouchPhase::Move, MouseWheelSource::Trackpad),
        ),
        (
            "a macOS wheel",
            wheel(-0.1, TouchPhase::Move, MouseWheelSource::Wheel),
        ),
        (
            "points",
            Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -3.5),
                phase: TouchPhase::Move,
                source: MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            },
        ),
    ];
    for (case, event) in cases {
        let mut events = vec![event.clone()];
        mark_precise_scroll(&mut events);
        assert_eq!(events, vec![event], "{case}");
    }
}

/// Touchpad scrolling stuttered when the fingers lifted while still moving (most visible in
/// settings): each fractional delta was eased in over ~100 ms like a wheel notch.
#[test]
fn mark_precise_scroll_applies_touchpad_deltas_in_the_frame_they_arrive() {
    let lines = -0.25;
    let eased = scrolled_per_frame(vec![vec![precise(lines)]], |_| {});
    let marked = scrolled_per_frame(vec![vec![precise(lines)]], mark_precise_scroll);
    assert!(
        eased[0] < -lines * LINE / 2.0,
        "egui eases it in: {eased:?}"
    );
    assert_eq!(marked, vec![-lines * LINE], "all of it lands at once");
}

/// A stream like the momentum after a touchpad lift: eased, the content trails it and keeps
/// moving after the last delta; marked, it moves with the stream and stops with it.
#[test]
fn mark_precise_scroll_moves_with_a_touchpad_stream_and_stops_with_it() {
    let stream = || {
        let mut frames: Vec<Vec<Event>> = [-0.3, -0.2, -0.15, -0.1]
            .into_iter()
            .map(|lines| vec![precise(lines)])
            .collect();
        frames.extend(std::iter::repeat_with(Vec::new).take(4));
        frames
    };
    let eased = scrolled_per_frame(stream(), |_| {});
    let marked = scrolled_per_frame(stream(), mark_precise_scroll);
    let after = |scrolled: &[f32]| scrolled[4..].iter().sum::<f32>();
    assert!(
        after(&eased) > 1.0,
        "eased, it trails the stream: {eased:?}"
    );
    assert_eq!(
        after(&marked),
        0.0,
        "marked, it stops with the stream: {marked:?}"
    );
}

#[test]
fn mark_precise_scroll_keeps_easing_mouse_wheel_notches() {
    let moved = scrolled_per_frame(vec![vec![precise(-1.0)]], mark_precise_scroll);
    assert!(
        0.0 < moved[0] && moved[0] < LINE / 2.0,
        "a notch still eases in: {moved:?}"
    );
}
