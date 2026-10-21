use eframe::egui::vec2;

use super::*;

#[test]
fn window_margin_is_nearly_full_bleed_when_narrow_and_grows_to_the_wide_margin() {
    let cases = [
        (360.0, 8.0),
        (600.0, 8.0),
        (660.0, 20.0),
        (720.0, 32.0),
        (1280.0, 32.0),
    ];
    for (extent, expected) in cases {
        let margin = window_margin(extent);
        assert!((margin - expected).abs() < 0.01, "{extent} pt: {margin}");
    }
}

/// With a stepped margin, widening a window past a step made its dialogs narrower.
#[test]
fn room_never_shrinks_as_the_window_grows() {
    let mut last = 0.0;
    for step in 0..=200 {
        let extent = 300.0 + 5.0 * step as f32;
        let room = room(vec2(extent, extent)).x;
        assert!(room >= last, "{extent} pt: {room} < {last}");
        last = room;
    }
}

#[test]
fn fit_keeps_a_dialog_inside_the_window() {
    let cases = [
        (
            vec2(1040.0, 780.0),
            vec2(1280.0, 760.0),
            vec2(1040.0, 696.0),
        ),
        (vec2(1040.0, 780.0), vec2(360.0, 640.0), vec2(344.0, 624.0)),
        (
            vec2(1040.0, 780.0),
            vec2(1280.0, 420.0),
            vec2(1040.0, 404.0),
        ),
        (vec2(1040.0, 780.0), vec2(720.0, 620.0), vec2(696.0, 596.0)),
        (vec2(420.0, 300.0), vec2(1280.0, 760.0), vec2(420.0, 300.0)),
        (vec2(420.0, 300.0), vec2(100.0, 10.0), vec2(84.0, 0.0)),
    ];
    for (want, window, expected) in cases {
        assert_eq!(fit(want, window), expected, "{want:?} in {window:?}");
    }
}

#[test]
fn rows_stack_below_a_label_column_beside_a_full_control() {
    assert_eq!(ROW_STACK_BELOW, 448.0);
    for (width, stacks) in [(312.0, true), (447.0, true), (448.0, false), (768.0, false)] {
        assert_eq!(row_stacks(width), stacks, "{width} pt");
    }
}

/// The column checklist was always three wide (512 pt), so at the smallest window its third
/// column ran past the page's right edge.
#[test]
fn per_line_fits_the_width() {
    let (item, gap) = (160.0, 16.0);
    let cases = [
        (600.0, 3),
        (512.0, 3),
        (511.0, 2),
        (384.0, 2),
        (336.0, 2),
        (335.0, 1),
        (100.0, 1),
    ];
    for (width, expected) in cases {
        let per_line = per_line(width, (item, gap), 3);
        assert_eq!(per_line, expected, "{width} pt");
        let used = per_line as f32 * (item + gap) - gap;
        assert!(per_line == 1 || used <= width, "{width} pt: {used} pt used");
    }
    assert_eq!(per_line(2000.0, (item, gap), 2), 2, "never more than asked");
    assert_eq!(per_line(2000.0, (item, gap), 0), 1, "always at least one");
}
