use super::*;

fn rect(x: f32, y: f32, w: f32, h: f32) -> WindowRect {
    WindowRect {
        x,
        y,
        w,
        h,
        maximized: false,
    }
}

#[test]
fn sane_rects_are_restored() {
    assert!(rect_plausible(&rect(100.0, 80.0, 1280.0, 760.0)));
    assert!(
        rect_plausible(&rect(-1900.0, 0.0, 1280.0, 760.0)),
        "a monitor left of the primary"
    );
}

#[test]
fn parked_or_degenerate_rects_are_not() {
    assert!(!rect_plausible(&rect(-32_000.0, -32_000.0, 1280.0, 760.0)));
    assert!(!rect_plausible(&rect(0.0, 0.0, 10.0, 10.0)));
    assert!(!rect_plausible(&rect(0.0, 0.0, 1e9, 760.0)));
}
