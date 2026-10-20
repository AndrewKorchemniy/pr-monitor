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
    assert_eq!(file.metrics, MetricTweaks::default());
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
    let src = format!("schema = 9\n{MINIMAL}\n[metrics]\nradius_scale = 7.0\n");
    let src = src.replacen("[seeds]\n", "[seeds]\ncontrast = -4.0\n", 1);
    let (file, issues) = parse_theme(&src, "mine").unwrap();
    assert!(issues.iter().any(|i| i.message.contains("newer")));
    assert!((file.metrics.radius_scale - 2.0).abs() < f32::EPSILON);
    assert!((file.seeds.contrast + 1.0).abs() < f32::EPSILON);
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
