use serde::Deserialize;

use super::*;

#[derive(Debug, Deserialize)]
struct Fields {
    #[serde(deserialize_with = "saturating_u8")]
    small: u8,
    #[serde(deserialize_with = "saturating_u32")]
    large: u32,
}

fn parse(small: i64, large: i64) -> Fields {
    serde_json::from_str(&format!(r#"{{"small": {small}, "large": {large}}}"#)).unwrap()
}

#[test]
fn saturating_ints_clamp_to_the_type_range() {
    for (small, large, expected) in [
        (7, 9, (7, 9)),
        (300, 5_000_000_000, (u8::MAX, u32::MAX)),
        (-1, -1, (0, 0)),
    ] {
        let fields = parse(small, large);
        assert_eq!((fields.small, fields.large), expected, "{small}, {large}");
    }
}

#[test]
fn saturating_ints_still_reject_non_integers() {
    let result = serde_json::from_str::<Fields>(r#"{"small": "five", "large": 1}"#);
    assert!(result.is_err());
}
