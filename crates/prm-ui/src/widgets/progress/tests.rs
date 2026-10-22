use super::{ProgressLine, SWEEP, Shown};
use crate::theme::{self, UiTheme};

/// One frame at 60 Hz, in seconds.
const FRAME: f64 = 1.0 / 60.0;

fn animated() -> UiTheme {
    let theme = UiTheme::clone(&theme::fallback());
    assert!(theme.policy.animations, "the fallback theme animates");
    theme
}

/// Ticks `line` once per frame for `frames` frames from `from`, with `target` throughout, and
/// returns what each frame showed.
fn run(
    line: &mut ProgressLine,
    theme: &UiTheme,
    (from, frames): (f64, usize),
    target: Option<f32>,
) -> Vec<Option<Shown>> {
    (0..frames)
        .map(|i| line.tick(target, from + i as f64 * FRAME, theme))
        .collect()
}

/// The line used to step to each report, ten times a second at most; now it fades in and
/// glides, moving a little every frame until it reaches the report.
#[test]
fn it_fades_in_and_glides_toward_the_target() {
    let theme = animated();
    let mut line = ProgressLine::default();
    let frames = run(&mut line, &theme, (0.0, 30), Some(0.5));
    let shown: Vec<Shown> = frames.into_iter().flatten().collect();
    assert_eq!(shown.len(), 30);
    assert_eq!(shown[0].fraction, 0.0);
    assert_eq!(shown[0].opacity, 0.0);
    assert!(
        shown[3].opacity > 0.4 && shown[3].opacity < 1.0,
        "{:?}",
        shown[3]
    );
    assert_eq!(shown[8].opacity, 1.0, "faded in within the quick motion");
    let rising = shown.windows(2).all(|p| p[1].fraction > p[0].fraction);
    assert!(rising, "every frame moves: {shown:?}");
    assert!(
        shown[29].fraction > 0.49 && shown[29].fraction <= 0.5,
        "{:?}",
        shown[29]
    );
}

/// Another org joining, or a phase reporting its own items, can lower the target: the line
/// holds where it is rather than running backwards.
#[test]
fn it_never_runs_backwards() {
    let theme = animated();
    let mut line = ProgressLine::default();
    run(&mut line, &theme, (0.0, 30), Some(0.6));
    let at = line.tick(Some(0.6), 0.5, &theme).map(|s| s.fraction);
    let after = run(&mut line, &theme, (0.5 + FRAME, 10), Some(0.2));
    for frame in after.into_iter().flatten() {
        assert!(Some(frame.fraction) >= at, "{frame:?} fell below {at:?}");
    }
}

/// Once the run is over the line does not blink out: it sweeps to the end, then fades.
#[test]
fn it_sweeps_to_the_end_and_fades_out_when_the_run_is_over() {
    let theme = animated();
    let mut line = ProgressLine::default();
    run(&mut line, &theme, (0.0, 30), Some(0.5));
    let over = run(&mut line, &theme, (0.5, 40), None);
    let shown: Vec<Shown> = over.iter().copied().flatten().collect();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a dozen frames"
    )]
    let sweep_frames = (f64::from(SWEEP) / FRAME) as usize;
    assert!(
        shown.len() > sweep_frames,
        "it is still shown after the sweep"
    );
    let during_sweep = &shown[..sweep_frames];
    assert!(
        during_sweep
            .windows(2)
            .all(|p| p[1].fraction >= p[0].fraction),
        "the sweep only goes forward"
    );
    assert!(
        during_sweep.iter().all(|s| s.opacity >= 1.0),
        "no fade before the end"
    );
    assert_eq!(
        shown[sweep_frames].fraction, 1.0,
        "the sweep reaches the end"
    );
    let fade = &shown[sweep_frames..];
    assert!(
        fade.windows(2).all(|p| p[1].opacity <= p[0].opacity),
        "then it fades"
    );
    assert!(over.last().is_some_and(Option::is_none), "and is gone");
}

/// A refresh that finishes at once still shows the line sweeping and fading: feedback for the
/// click, not a flash.
#[test]
fn an_instant_run_is_still_acknowledged() {
    let theme = animated();
    let mut line = ProgressLine::default();
    assert!(line.tick(Some(0.0), 0.0, &theme).is_some());
    let over = run(&mut line, &theme, (FRAME, 40), None);
    let visible = over.iter().filter(|f| f.is_some()).count();
    assert!(
        visible as f64 * FRAME >= f64::from(SWEEP),
        "shown {visible} frames"
    );
    assert!(over.last().is_some_and(Option::is_none));
}

/// A run starting while the last one fades out waits for it, then starts afresh.
#[test]
fn a_run_during_the_fade_out_waits_for_it() {
    let theme = animated();
    let mut line = ProgressLine::default();
    run(&mut line, &theme, (0.0, 30), Some(0.5));
    run(&mut line, &theme, (0.5, 3), None);
    let mut restarted = false;
    let mut last = 0.0;
    for i in 0..60 {
        let Some(shown) = line.tick(Some(0.3), 0.6 + f64::from(i) * FRAME, &theme) else {
            continue;
        };
        if shown.fraction < last {
            assert!(
                !restarted,
                "it may start over once, not run backwards: {shown:?}"
            );
            restarted = true;
            assert_eq!(shown.fraction, 0.0, "it starts afresh from the left");
        }
        last = shown.fraction;
    }
    assert!(restarted, "the new run started once the fade was done");
}

/// With animations off (or low-power mode) the line snaps to each report and vanishes with the
/// run, and asks for no repaints of its own.
#[test]
fn with_animations_off_it_snaps_and_vanishes() {
    let mut theme = animated();
    theme.policy.animations = false;
    let mut line = ProgressLine::default();
    assert_eq!(
        line.tick(Some(0.4), 0.0, &theme),
        Some(Shown {
            fraction: 0.4,
            opacity: 1.0,
            moving: false,
        })
    );
    assert_eq!(line.tick(None, FRAME, &theme), None);
}

/// Between reports the line sits still, and then it must not keep the window repainting at
/// 60 Hz for the length of a refresh: once the fade is in and the glide has arrived, it stops
/// asking, and the next report wakes it through the usual event path.
#[test]
fn it_asks_for_repaints_only_while_it_moves() {
    let theme = animated();
    let mut line = ProgressLine::default();
    let frames = run(&mut line, &theme, (0.0, 90), Some(0.5));
    let shown: Vec<Shown> = frames.into_iter().flatten().collect();
    assert!(shown[0].moving && shown[10].moving, "fading in and gliding");
    assert!(!shown[89].moving, "settled: {:?}", shown[89]);
    let settled = shown.iter().position(|s| !s.moving).expect("it settles");
    assert!(
        shown[settled..].iter().all(|s| !s.moving),
        "and stays settled"
    );
    let resumed = line.tick(Some(0.8), 1.6, &theme).expect("shown");
    assert!(resumed.moving, "a new report sets it moving again");
    let over = run(&mut line, &theme, (1.7, 30), None);
    assert!(
        over.iter().flatten().all(|s| s.moving),
        "the sweep and fade keep it moving"
    );
}
