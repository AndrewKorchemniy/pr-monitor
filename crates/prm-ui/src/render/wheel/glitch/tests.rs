use eframe::egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, TouchPhase, pos2, vec2};

use super::GlitchFilter;

fn precise(lines: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, lines),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn delta_y(event: &Event) -> Option<f32> {
    if let Event::MouseWheel { delta, .. } = event {
        Some(delta.y)
    } else {
        None
    }
}

/// Feeds each frame's deltas through the filter and returns the wheel deltas out per frame.
fn run(frames: &[&[f32]]) -> Vec<Vec<f32>> {
    let mut filter = GlitchFilter::default();
    frames
        .iter()
        .map(|frame| {
            let mut events = vec![Event::PointerMoved(pos2(1.0, 1.0))];
            events.extend(frame.iter().map(|lines| precise(*lines)));
            filter.apply(&mut events);
            events.iter().filter_map(delta_y).collect()
        })
        .collect()
}

/// Recorded at the top of the settings page: pushes of +2.4 lines, one -0.53 among them, and
/// the page jumped 21 pt down for a frame.
#[test]
fn a_lone_small_reversal_inside_a_run_is_dropped() {
    let out = run(&[&[2.45], &[2.4], &[2.6], &[-0.53], &[2.44], &[5.86]]);
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
fn a_lone_reversal_against_a_downward_run_is_dropped_too() {
    let out = run(&[&[-0.98], &[-1.05], &[-1.08], &[-1.02], &[0.37], &[-1.37]]);
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

/// The user turning round: the reversal is followed by more of the same, and all of it lands,
/// the first one a frame late.
#[test]
fn a_real_change_of_direction_is_kept() {
    let out = run(&[&[-1.1], &[-1.1], &[-1.1], &[0.4], &[0.8], &[1.2]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![-1.1],
        vec![-1.1],
        vec![-1.1],
        vec![],
        vec![0.4, 0.8],
        vec![1.2],
    ];
    assert_eq!(out, expected);
}

/// A reversal that is the last event of a stream is still delivered on the first frame with no
/// wheel input: holding it would lose a real, if small, scroll.
#[test]
fn a_reversal_that_ends_the_stream_is_delivered_when_nothing_follows() {
    let out = run(&[&[-1.1], &[-1.1], &[-1.1], &[0.4], &[], &[]]);
    let expected: Vec<Vec<f32>> = vec![
        vec![-1.1],
        vec![-1.1],
        vec![-1.1],
        vec![],
        vec![0.4],
        vec![],
    ];
    assert_eq!(out, expected);
}

/// A large reversal is a deliberate flick back, never noise.
#[test]
fn a_large_reversal_passes_at_once() {
    let out = run(&[&[-1.1], &[-1.1], &[-1.1], &[3.3]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.1], vec![-1.1], vec![-1.1], vec![3.3]];
    assert_eq!(out, expected);
}

/// Before a run is established, nothing is held back.
#[test]
fn early_reversals_pass() {
    let out = run(&[&[-1.1], &[0.5], &[-1.1]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.1], vec![0.5], vec![-1.1]];
    assert_eq!(out, expected);
}

/// Several events in one frame go through the same logic.
#[test]
fn works_within_a_frame() {
    let out = run(&[&[2.2, 2.2, 2.2, -0.3, 2.2]]);
    let expected: Vec<Vec<f32>> = vec![vec![2.2, 2.2, 2.2, 2.2]];
    assert_eq!(out, expected);
}

/// Whole notches from a mouse wheel are not touchpad events: they end the run and pass.
#[test]
fn mouse_notches_are_left_alone() {
    let out = run(&[&[-1.0], &[-1.0], &[-1.0], &[1.0], &[-1.0]]);
    let expected: Vec<Vec<f32>> = vec![vec![-1.0], vec![-1.0], vec![-1.0], vec![1.0], vec![-1.0]];
    assert_eq!(out, expected);
}
