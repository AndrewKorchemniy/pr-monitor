use super::*;

#[test]
fn secret_debug_is_redacted() {
    let s = Secret::new("hunter2");
    assert_eq!(format!("{s:?}"), "Secret(***)");
    assert_eq!(format!("{:?}", Some(s.clone())), "Some(Secret(***))");
    assert_eq!(s.expose(), "hunter2");
}

#[test]
fn token_id_stable() {
    assert_eq!(Secret::new("abc").id(), Secret::new("abc").id());
    assert_ne!(Secret::new("abc").id(), Secret::new("abd").id());
    assert_eq!(Secret::new("a").id(), TokenId(0xaf63_dc4c_8601_ec8c));
}
