use super::*;

#[test]
fn probe_covers_the_title_bar_at_every_scale() {
    // 100 points at scale 1..3 is 100..300 pixels; plus a 200-wide, 30-high strip.
    assert_eq!(title_bar_probe(100.0, 50.0, 1280.0), [100, 50, 500, 180]);
}

#[test]
fn probe_strip_is_no_wider_than_the_window() {
    assert_eq!(title_bar_probe(0.0, 0.0, 120.0), [0, 0, 120, 30]);
}

#[test]
fn probe_extends_left_for_a_monitor_left_of_the_primary() {
    assert_eq!(title_bar_probe(-1000.0, 0.0, 800.0), [-3000, 0, -800, 30]);
}

#[test]
fn probe_saturates_instead_of_wrapping() {
    let [left, _, right, _] = title_bar_probe(f32::MAX, 0.0, 800.0);
    assert_eq!((left, right), (i32::MAX, i32::MAX));
}
