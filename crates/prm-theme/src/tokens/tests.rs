use super::*;
use crate::{builtin::builtin_sources, color::Rgba8, file::parse_theme};

fn graphite() -> (ThemeFile, &'static str) {
    let (stem, src) = builtin_sources()
        .iter()
        .find(|(stem, _)| *stem == "graphite")
        .unwrap();
    (parse_theme(src, stem).unwrap().0, src)
}

#[test]
fn override_unknown_key_warns() {
    let (mut file, src) = graphite();
    file.overrides.insert("row.nope".into(), Rgba8::WHITE);
    let (_, issues) = Tokens::build(&file, src, ThemeSource::Builtin);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Warning);
    assert!(issues[0].message.contains("row.nope"));
}

#[test]
fn override_applies_then_validates() {
    let (mut file, src) = graphite();
    let selected = Rgba8::rgb(0x25, 0x31, 0x3A);
    file.overrides.insert("row.selected".into(), selected);
    file.overrides
        .insert("text.primary".into(), Rgba8::rgb(0x30, 0x30, 0x30));
    let (tokens, issues) = Tokens::build(&file, src, ThemeSource::Builtin);
    assert_eq!(tokens.palette.row_selected, selected);
    assert!(
        issues
            .iter()
            .any(|i| i.key == Some("text.primary") && i.level == IssueLevel::Error)
    );
}

#[test]
fn build_carries_identity_and_metrics() {
    let (file, src) = graphite();
    let (tokens, issues) = Tokens::build(&file, src, ThemeSource::Builtin);
    assert_eq!(issues, Vec::new());
    assert_eq!(tokens.id.as_str(), "graphite");
    assert_eq!(tokens.appearance, Appearance::Dark);
    assert_eq!(tokens.metrics, Metrics::new(&file.metrics));
    assert_eq!(tokens.file_toml.as_ref(), src);
}
