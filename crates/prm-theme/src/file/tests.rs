use super::*;

const MINIMAL: &str = r##"
id = "mine"
name = "Mine"
appearance = "light"

[seeds]
base = "#FFFFFF"
ink = "#111111"
accent = "#1D6CB0"

[seeds.status]
needs_review = "#B07A0A"
active_comments = "#C2502F"
reviewed = "#2E7D52"

[seeds.feedback]
danger = "#C2334B"
warning = "#B07A0A"
"##;

#[test]
fn parse_minimal_fills_defaults() {
    let (file, issues) = parse_theme(MINIMAL, "mine").unwrap();
    assert_eq!(issues, Vec::new());
    assert_eq!(file.schema, THEME_SCHEMA);
    assert_eq!(file.seeds.feedback.success, None);
    assert!(file.overrides.is_empty());
}

#[test]
fn id_mismatch_uses_file_stem() {
    let (file, issues) = parse_theme(MINIMAL, "renamed").unwrap();
    assert_eq!(file.id, "renamed");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Warning);
}

#[test]
fn newer_schema_warns_and_clamps() {
    let src = format!("schema = 9\n{MINIMAL}");
    let src = src.replacen("[seeds]\n", "[seeds]\ncontrast = -4.0\n", 1);
    let (file, issues) = parse_theme(&src, "mine").unwrap();
    assert!(issues.iter().any(|i| i.message.contains("newer")));
    assert!((file.seeds.contrast + 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_leftover_radius_scale_parses_with_one_info_issue() {
    let src = format!("{MINIMAL}\n[metrics]\nradius_scale = 0.0\n");
    let (_, issues) = parse_theme(&src, "mine").unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Info);
    assert_eq!(issues[0].message, RADIUS_SCALE_RETIRED);
    assert!(
        issues[0]
            .message
            .contains("Settings › Appearance › Corners")
    );
}

#[test]
fn a_leftover_stripes_key_parses_with_one_info_issue() {
    let src = format!("{MINIMAL}\n[metrics]\nstripes = false\n");
    let (_, issues) = parse_theme(&src, "mine").unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].level, IssueLevel::Info);
    assert_eq!(issues[0].message, STRIPES_RETIRED);
    assert_eq!(
        STRIPES_RETIRED,
        "stripes is no longer a theme setting; use Settings › Appearance › Striped table rows"
    );
}

#[test]
fn every_retired_key_gets_its_own_issue() {
    let src = format!("{MINIMAL}\n[metrics]\nradius_scale = 0.0\nstripes = true\n");
    let (_, issues) = parse_theme(&src, "mine").unwrap();
    let messages: Vec<_> = issues.iter().map(|i| i.message.as_str()).collect();
    assert_eq!(messages, [RADIUS_SCALE_RETIRED, STRIPES_RETIRED]);
}

#[test]
fn retired_keys_elsewhere_are_not_flagged() {
    let src = format!("radius_scale = 1.0\nstripes = true\n{MINIMAL}");
    assert_eq!(parse_theme(&src, "mine").unwrap().1, Vec::new());
}

#[test]
fn bad_hex_is_a_parse_error() {
    let src = MINIMAL.replace("#111111", "#12");
    let Err(ThemeError::Parse { file, message }) = parse_theme(&src, "mine") else {
        panic!("expected a parse error");
    };
    assert_eq!(file, "mine.toml");
    assert!(message.contains("#12"), "{message}");
}

#[test]
fn unknown_keys_are_ignored() {
    let src = format!("future_key = 1\n{MINIMAL}");
    assert!(parse_theme(&src, "mine").is_ok());
}
