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

fn touchpad(lines: f32) -> Event {
    wheel(lines, TouchPhase::Move, MouseWheelSource::Unknown)
}

fn start() -> Event {
    wheel(0.0, TouchPhase::Start, MouseWheelSource::Unknown)
}

/// Feeds each frame's touchpad deltas (after a pointer move) through one filter and returns
/// the non-zero wheel deltas that came out of each frame.
fn filtered(frames: &[&[f32]]) -> Vec<Vec<f32>> {
    let mut scroll = TouchpadScroll::default();
    frames
        .iter()
        .map(|frame| {
            let mut events = vec![Event::PointerMoved(pos2(1.0, 1.0))];
            events.extend(frame.iter().map(|lines| touchpad(*lines)));
            scroll.rewrite(&mut events);
            events
                .iter()
                .filter_map(|e| {
                    if let Event::MouseWheel { delta, .. } = e {
                        Some(delta.y)
                    } else {
                        None
                    }
                })
                .filter(|y| *y != 0.0)
                .collect()
        })
        .collect()
}

/// How far egui scrolls in each frame, one frame per entry of `frames`, with `prepare` run on
/// each frame's events first: the `smooth_scroll_delta` every scroll area applies.
fn scrolled_per_frame(
    frames: Vec<Vec<Event>>,
    mut prepare: impl FnMut(&mut Vec<Event>),
) -> Vec<f32> {
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

fn rewritten() -> impl FnMut(&mut Vec<Event>) {
    let mut scroll = TouchpadScroll::default();
    move |events| scroll.rewrite(events)
}

#[test]
fn a_gesture_starts_before_the_first_touchpad_delta() {
    let mut events = vec![
        Event::PointerMoved(pos2(1.0, 1.0)),
        touchpad(-0.1),
        touchpad(-0.2),
    ];
    TouchpadScroll::default().rewrite(&mut events);
    let expected = vec![
        Event::PointerMoved(pos2(1.0, 1.0)),
        start(),
        touchpad(-0.1),
        touchpad(-0.2),
    ];
    assert_eq!(events, expected);
}

#[test]
fn other_wheel_input_is_left_alone() {
    let cases = [
        ("a whole notch", touchpad(-1.0)),
        ("two notches", touchpad(2.0)),
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
        TouchpadScroll::default().rewrite(&mut events);
        assert_eq!(events, vec![event], "{case}");
    }
}

/// Touchpad scrolling stuttered when the fingers lifted while still moving (most visible in
/// settings): each fractional delta was eased in over ~100 ms like a wheel notch.
#[test]
fn touchpad_deltas_apply_in_the_frame_they_arrive() {
    let lines = -0.25;
    let eased = scrolled_per_frame(vec![vec![touchpad(lines)]], |_| {});
    let marked = scrolled_per_frame(vec![vec![touchpad(lines)]], rewritten());
    assert!(
        eased[0] < -lines * LINE / 2.0,
        "egui eases it in: {eased:?}"
    );
    assert_eq!(marked, vec![-lines * LINE], "all of it lands at once");
}

/// A stream like the momentum after a touchpad lift: eased, the content trails it and keeps
/// moving after the last delta; marked, it moves with the stream and stops with it.
#[test]
fn the_page_moves_with_a_touchpad_stream_and_stops_with_it() {
    let stream = || {
        let mut frames: Vec<Vec<Event>> = [-0.3, -0.2, -0.15, -0.1]
            .into_iter()
            .map(|lines| vec![touchpad(lines)])
            .collect();
        frames.extend(std::iter::repeat_with(Vec::new).take(4));
        frames
    };
    let eased = scrolled_per_frame(stream(), |_| {});
    let marked = scrolled_per_frame(stream(), rewritten());
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
fn mouse_wheel_notches_keep_easing() {
    let moved = scrolled_per_frame(vec![vec![touchpad(-1.0)]], rewritten());
    assert!(
        0.0 < moved[0] && moved[0] < LINE / 2.0,
        "a notch still eases in: {moved:?}"
    );
}

/// Recorded at the top of the settings page: pushes of +2.4 lines, one -0.53 among them, and
/// the page jumped 21 pt down for a frame.
#[test]
fn a_lone_small_reversal_inside_a_stream_is_dropped() {
    let out = filtered(&[&[2.45], &[2.4], &[2.6], &[-0.53], &[2.44], &[5.86]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![2.45],
        vec![2.4],
        vec![2.6],
        vec![],
        vec![2.44],
        vec![5.86],
    ];
    assert_eq!(out, expected);
}

/// Recorded at the bottom: -1.0s, then one +0.37, then -1.37.
#[test]
fn a_lone_reversal_against_a_downward_stream_is_dropped_too() {
    let out = filtered(&[&[-0.98], &[-1.05], &[-1.08], &[-1.02], &[0.37], &[-1.37]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![-0.98],
        vec![-1.05],
        vec![-1.08],
        vec![-1.02],
        vec![],
        vec![-1.37],
    ];
    assert_eq!(out, expected);
}

/// Recorded as one finger lifted at an edge: a spike of several lines the other way, then a
/// small step back the old way. Held until that step arrives, the spike is dropped.
#[test]
fn the_spike_at_a_finger_lift_is_dropped() {
    let out = filtered(&[&[-1.08], &[-1.1], &[-1.05], &[5.23], &[-1.26], &[-0.9]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![-1.08],
        vec![-1.1],
        vec![-1.05],
        vec![],
        vec![-1.26],
        vec![-0.9],
    ];
    assert_eq!(out, expected);
}

/// The spike and its follower often share a frame.
#[test]
fn the_spike_is_dropped_within_a_frame_too() {
    let out = filtered(&[&[-1.1], &[-1.1], &[-1.1, 24.98, -0.54]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.1], vec![-1.1], vec![-1.1, -0.54]];
    assert_eq!(out, expected);
}

/// The user turning round: a second reversed event confirms the turn, and the first goes out
/// ahead of it, one frame late.
#[test]
fn a_real_change_of_direction_passes_in_full() {
    let out = filtered(&[&[-1.1], &[-1.1], &[-1.1], &[3.3], &[2.1], &[1.2]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![-1.1],
        vec![-1.1],
        vec![-1.1],
        vec![],
        vec![3.3, 2.1],
        vec![1.2],
    ];
    assert_eq!(out, expected);
}

/// A reversal that ends the stream is held until the touchpad has been quiet for a while, then
/// released: a real, if small, scroll is not lost, and a stray at the end does no harm.
#[test]
fn a_reversal_that_ends_the_stream_is_released_once_the_stream_is_over() {
    let mut frames: Vec<&[f32]> = vec![&[-1.1], &[-1.1], &[-1.1], &[0.4]];
    frames.extend(std::iter::repeat_n(&[][..], IDLE_FRAMES as usize));
    let quiet = filtered(&frames);
    assert!(
        quiet[3..].iter().all(Vec::is_empty),
        "still held: {quiet:?}"
    );
    frames.push(&[]);
    let released = filtered(&frames);
    assert_eq!(released.last(), Some(&vec![0.4]));
}

/// Before a stream is established, nothing is held.
#[test]
fn early_reversals_pass() {
    let out = filtered(&[&[-1.1], &[0.5], &[-1.1]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.1], vec![0.5], vec![-1.1]];
    assert_eq!(out, expected);
}

/// Several events in one frame go through the same logic.
#[test]
fn the_filter_works_within_a_frame() {
    let out = filtered(&[&[2.2, 2.2, 2.2, -0.3, 2.2]]);
    let expected: Vec<Vec<f32>> = vec![vec![2.2, 2.2, 2.2, 2.2]];
    assert_eq!(out, expected);
}

/// Once the touchpad has been quiet for a while the stream is over, and the first event of a
/// new one is never taken for a glitch, whichever way it goes.
#[test]
fn a_new_stream_after_a_pause_starts_either_way() {
    let mut frames: Vec<&[f32]> = vec![&[-1.1], &[-1.1], &[-1.1]];
    frames.extend(std::iter::repeat_n(&[][..], IDLE_FRAMES as usize + 1));
    frames.push(&[0.4]);
    let out = filtered(&frames);
    assert_eq!(out.last(), Some(&vec![0.4]));
}

/// Whole notches from a mouse wheel are not touchpad events and are never held or dropped.
#[test]
fn mouse_notches_are_never_dropped() {
    let out = filtered(&[&[-1.0], &[-1.0], &[-1.0], &[1.0], &[-1.0]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.0], vec![-1.0], vec![-1.0], vec![1.0], vec![-1.0]];
    assert_eq!(out, expected);
}
