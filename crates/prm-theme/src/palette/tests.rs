use std::collections::HashSet;

use super::*;

#[test]
fn palette_keys_unique_and_dotted() {
    let unique: HashSet<&str> = Palette::KEYS.iter().copied().collect();
    assert_eq!(unique.len(), Palette::KEYS.len());
    assert_eq!(Palette::KEYS.len(), 63);
    for key in Palette::KEYS {
        assert!(key.contains('.'), "{key}");
        assert!(
            key.bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_'),
            "{key}"
        );
    }
}

#[test]
fn palette_get_set_by_key() {
    let tokens = crate::ThemeRegistry::builtin()
        .tokens("graphite")
        .unwrap()
        .0;
    let mut palette = tokens.palette;
    assert_eq!(palette.get("row.hover"), Some(palette.row_hover));
    assert!(palette.set("row.hover", Rgba8::WHITE));
    assert_eq!(palette.row_hover, Rgba8::WHITE);
    assert!(!palette.set("row.nope", Rgba8::WHITE));
    assert_eq!(palette.get("row.nope"), None);
    assert_eq!(palette.iter().count(), Palette::KEYS.len());
}
