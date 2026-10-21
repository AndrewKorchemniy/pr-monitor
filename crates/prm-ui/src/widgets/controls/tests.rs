use super::MenuMark;

#[test]
fn a_key_driven_list_fills_only_its_highlighted_row() {
    let lit = MenuMark::Keys { lit: true };
    let unlit = MenuMark::Keys { lit: false };
    assert!(lit.filled(false) && lit.current());
    assert!(!unlit.filled(true), "hovering does not fill a second row");
    assert!(!unlit.current());

    let plain = MenuMark::Pointer { selected: false };
    assert!(plain.filled(true) && !plain.filled(false) && !plain.current());
    let selected = MenuMark::Pointer { selected: true };
    assert!(selected.filled(false) && selected.current());
}
