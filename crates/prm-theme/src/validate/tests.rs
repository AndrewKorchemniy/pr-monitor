use super::*;
use crate::{ThemeRegistry, color::Rgba8};

fn graphite() -> Palette {
    ThemeRegistry::builtin()
        .tokens("graphite")
        .unwrap()
        .0
        .palette
}

#[test]
fn rules_reference_real_keys() {
    for rule in CONTRAST_RULES {
        assert!(Palette::KEYS.contains(&rule.fg), "{}", rule.fg);
        for bg in rule.bg {
            assert!(Palette::KEYS.contains(bg), "{bg}");
        }
    }
}

#[test]
fn validate_flags_low_contrast_by_severity() {
    let id = ThemeId::new("test");
    let mut palette = graphite();
    palette.text_muted = palette.surface_sunken;
    let issues = validate(&id, &palette);
    assert!(
        issues
            .iter()
            .any(|i| i.key == Some("text.muted") && i.level == IssueLevel::Error)
    );

    let mut palette = graphite();
    palette.text_muted = Rgba8::rgb(0x5A, 0x5D, 0x62);
    let issues = validate(&id, &palette);
    let muted: Vec<_> = issues
        .iter()
        .filter(|i| i.key == Some("text.muted"))
        .collect();
    assert_ne!(muted.len(), 0);
    assert!(
        muted.iter().all(|i| i.level == IssueLevel::Warning),
        "{muted:?}"
    );
}

#[test]
fn validate_clean_palette_has_no_issues() {
    assert_eq!(validate(&ThemeId::new("graphite"), &graphite()), Vec::new());
}
