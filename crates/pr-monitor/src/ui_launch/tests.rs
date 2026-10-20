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

#[test]
fn a_rect_off_every_monitor_is_not_restored() {
    let saved = rect(100.0, 80.0, 1280.0, 760.0);
    assert!(rect_on_screen(&saved, |_| Some(true)));
    assert!(rect_on_screen(&saved, |_| None), "unknown means trust it");
    assert!(!rect_on_screen(&saved, |_| Some(false)));
}

#[test]
fn an_implausible_rect_never_asks_the_os() {
    let parked = rect(-32_000.0, -32_000.0, 1280.0, 760.0);
    assert!(!rect_on_screen(&parked, |_| unreachable!("short-circuits")));
}
