use super::*;

#[test]
fn a_window_rect_knows_its_top_left_corner() {
    let rect = WindowRect {
        x: -1900.0,
        y: 40.0,
        w: 1280.0,
        h: 760.0,
        maximized: false,
    };
    assert_eq!(rect.position(), [-1900.0, 40.0]);
}
