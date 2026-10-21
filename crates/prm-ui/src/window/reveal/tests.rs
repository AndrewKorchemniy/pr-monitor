use super::*;

/// Runs frames of `logic` then (when `ui_runs`) a ui pass, collecting the frames that uncloak.
fn frames(mut reveal: Reveal, count: usize, ui_runs: bool) -> (Reveal, Vec<usize>) {
    let mut uncloaked = Vec::new();
    for frame in 0..count {
        if reveal.begin_logic() {
            uncloaked.push(frame);
        }
        if ui_runs {
            reveal.painted();
        }
    }
    (reveal, uncloaked)
}

/// The white flash on launch: eframe shows the window at the end of frame 0, so the frame
/// painted then never counts; frame 1 paints on screen and frame 2 uncloaks.
#[test]
fn reveal_launch_uncloaks_after_the_first_frame_painted_on_screen() {
    let (reveal, uncloaked) = frames(Reveal::Launching, 4, true);

    assert_eq!(uncloaked, vec![2]);
    assert_eq!(reveal, Reveal::Idle);
}

/// The white flash on tray open: the frame whose `logic` asked for the show paints into the
/// still-hidden window, so it does not count either.
#[test]
fn reveal_show_ignores_the_paint_of_the_frame_that_showed_it() {
    let mut reveal = Reveal::Idle;
    assert!(!reveal.begin_logic());
    reveal = Reveal::Showing;
    reveal.painted();
    assert_eq!(reveal, Reveal::Showing);

    let (reveal, uncloaked) = frames(reveal, 3, true);

    assert_eq!(uncloaked, vec![1]);
    assert_eq!(reveal, Reveal::Idle);
}

#[test]
fn reveal_waits_for_a_ui_pass_however_many_logic_passes_run() {
    let (reveal, uncloaked) = frames(Reveal::Showing, 5, false);
    assert_eq!(uncloaked, Vec::<usize>::new());
    assert_eq!(reveal, Reveal::AwaitPaint);

    let (reveal, uncloaked) = frames(reveal, 2, true);

    assert_eq!(uncloaked, vec![1]);
    assert_eq!(reveal, Reveal::Idle);
}

#[test]
fn reveal_idle_never_uncloaks() {
    let (reveal, uncloaked) = frames(Reveal::Idle, 3, true);

    assert_eq!(uncloaked, Vec::<usize>::new());
    assert_eq!(reveal, Reveal::Idle);
}

#[test]
fn reveal_minimized_uncloaks_only_once_the_window_is_on_screen() {
    let cases = [
        (Reveal::Idle, false),
        (Reveal::Launching, false),
        (Reveal::Showing, false),
        (Reveal::AwaitPaint, true),
        (Reveal::AwaitPresent, true),
    ];
    for (start, uncloak) in cases {
        let mut reveal = start;

        let got = reveal.minimized();

        assert_eq!(got, uncloak, "{start:?}");
        let rest = if uncloak { Reveal::Idle } else { start };
        assert_eq!(reveal, rest, "{start:?}");
    }
}

#[test]
fn reveal_needs_a_frame_only_when_nothing_else_would_bring_one() {
    let cases = [
        (Reveal::Idle, false),
        (Reveal::Launching, false),
        (Reveal::Showing, true),
        (Reveal::AwaitPaint, false),
        (Reveal::AwaitPresent, true),
    ];
    for (reveal, needs) in cases {
        assert_eq!(reveal.needs_frame(), needs, "{reveal:?}");
    }
}
