use super::*;

/// `#[derive(Default)]` gave a dialog width 0, which then shrank to nothing in any window.
#[test]
fn a_new_spec_is_usually_wide_and_has_no_buttons() {
    let spec = ModalSpec::new("id", "Title");

    assert_eq!(spec.width, WIDTH);
    assert!(spec.buttons.is_empty());
    assert!(!spec.danger && !spec.nested);
}
