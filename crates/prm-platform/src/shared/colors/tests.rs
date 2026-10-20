use super::*;

#[test]
fn colorref_is_bgr() {
    let color = Rgb {
        r: 0x16,
        g: 0x18,
        b: 0x1C,
    };
    assert_eq!(colorref(color), 0x001C_1816);
}
