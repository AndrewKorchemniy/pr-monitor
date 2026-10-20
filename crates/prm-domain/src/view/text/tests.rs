use super::*;

#[test]
fn truncate_chars_respects_char_boundaries() {
    assert_eq!(truncate_chars("héllo", 10), "héllo");
    assert_eq!(truncate_chars("héllo wörld", 5), "héll…");
    assert_eq!(truncate_chars("", 3), "");
}

#[test]
fn truncate_utf16_never_splits_surrogates() {
    let emoji = "ab😀cd";
    // a, b, 😀 (2 units), c, d = 6 units.
    assert_eq!(truncate_utf16(emoji, 6), emoji);
    assert_eq!(truncate_utf16(emoji, 4), "ab…");
    assert!(truncate_utf16(emoji, 5).encode_utf16().count() <= 5);
}

#[test]
fn initials_table() {
    for (name, expected) in [
        ("Jane Doe", "JD"),
        ("jane", "J"),
        ("[Payments]\\Payments Team", "PT"),
        ("Ann-Marie O'Neil", "AM"),
        ("", ""),
    ] {
        assert_eq!(initials(name), expected, "{name}");
    }
}

#[test]
fn strip_ref_prefix_only_strips_heads() {
    assert_eq!(strip_ref_prefix("refs/heads/feature/x"), "feature/x");
    assert_eq!(strip_ref_prefix("main"), "main");
}

#[test]
fn snippet_takes_first_visible_line() {
    assert_eq!(snippet("\n\n> quoted\nsecond").as_deref(), Some("quoted"));
    assert_eq!(snippet("## Heading  \nbody").as_deref(), Some("Heading"));
    assert_eq!(snippet("   \n  "), None);
    assert_eq!(
        snippet(&"x".repeat(200)).map(|s| s.chars().count()),
        Some(MAX_SNIPPET_CHARS)
    );
}
