use std::fs;

use super::*;

const USER_GRAPHITE: &str = r##"
id = "graphite"
name = "My graphite"
appearance = "dark"

[seeds]
base = "#101010"
ink = "#F0F0F0"
accent = "#86C8EA"

[seeds.status]
needs_review = "#E7B551"
active_comments = "#EF8467"
reviewed = "#82C79D"

[seeds.feedback]
danger = "#F2697C"
warning = "#E7B551"
"##;

fn selection(theme: &str) -> ThemeSelection {
    ThemeSelection {
        theme: theme.into(),
        dark: DEFAULT_DARK.into(),
        light: DEFAULT_LIGHT.into(),
        density: Density::Compact,
    }
}

#[test]
fn user_theme_replaces_builtin_info() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("graphite.toml"), USER_GRAPHITE).unwrap();

    let (registry, issues) = ThemeRegistry::builtin().with_user_dir(dir.path());

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Info);
    let entry = registry.get("graphite").unwrap();
    assert_eq!(entry.file.name, "My graphite");
    assert!(matches!(entry.source, ThemeSource::User(_)));
}

#[test]
fn user_theme_bad_hex_is_error_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let broken = USER_GRAPHITE.replace("#101010", "#10");
    fs::write(dir.path().join("broken.toml"), broken).unwrap();

    let (registry, issues) = ThemeRegistry::builtin().with_user_dir(dir.path());

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Error);
    assert_eq!(issues[0].theme.as_str(), "broken");
    assert!(registry.get("broken").is_none());
}

#[test]
fn missing_user_dir_is_fine() {
    let dir = tempfile::tempdir().unwrap();
    let (registry, issues) = ThemeRegistry::builtin().with_user_dir(&dir.path().join("nope"));
    assert_eq!(issues, Vec::new());
    assert_eq!(registry.list().count(), 3);
}

#[test]
fn resolve_system_uses_dark_and_light_picks() {
    let set = resolve(&ThemeRegistry::builtin(), &selection(SYSTEM));
    assert!(set.follow_system);
    assert_eq!(set.dark.id.as_str(), "graphite");
    assert_eq!(set.light.id.as_str(), "vellum");
    assert_eq!(set.issues, Vec::new());
}

#[test]
fn resolve_explicit_fills_both_slots() {
    let set = resolve(&ThemeRegistry::builtin(), &selection("cyanotype"));
    assert!(!set.follow_system);
    assert_eq!(set.dark.id.as_str(), "cyanotype");
    assert_eq!(set.light.id.as_str(), "cyanotype");
}

#[test]
fn resolve_unknown_falls_back_with_warning() {
    let mut picks = selection(SYSTEM);
    picks.light = "nope".into();
    let set = resolve(&ThemeRegistry::builtin(), &picks);
    assert_eq!(set.light.id.as_str(), DEFAULT_LIGHT);
    assert_eq!(set.issues.len(), 1);
    assert_eq!(set.issues[0].level, IssueLevel::Warning);

    let set = resolve(&ThemeRegistry::builtin(), &selection("nope"));
    assert_eq!(set.dark.id.as_str(), DEFAULT_DARK);
}

#[test]
fn last_resort_builds_first_builtin() {
    let (tokens, issues) = last_resort(Density::Compact);
    assert_eq!(tokens.id.as_str(), builtin_sources()[0].0);
    assert_eq!(issues, Vec::new());
}
