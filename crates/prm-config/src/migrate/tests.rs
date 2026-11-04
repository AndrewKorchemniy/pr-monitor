use super::*;

#[expect(
    clippy::unnecessary_wraps,
    reason = "must match the Migration signature"
)]
fn add_marker(table: &mut toml::Table) -> Result<(), String> {
    let steps = table
        .get("steps")
        .and_then(toml::Value::as_integer)
        .unwrap_or(0);
    table.insert("steps".into(), toml::Value::Integer(steps + 1));
    Ok(())
}

fn fail(_: &mut toml::Table) -> Result<(), String> {
    Err("boom".into())
}

#[test]
fn migrate_with_runs_in_order() {
    let mut table = toml::Table::new();
    let version = migrate_with(&mut table, 1, 3, &[(1, add_marker), (2, add_marker)]).unwrap();
    assert_eq!(version, 3);
    assert_eq!(
        table.get("steps").and_then(toml::Value::as_integer),
        Some(2)
    );
    assert_eq!(
        table
            .get("schema_version")
            .and_then(toml::Value::as_integer),
        Some(3)
    );
}

#[test]
fn migrate_failure_reports_error() {
    let mut table = toml::Table::new();
    let err = migrate_with(&mut table, 1, 2, &[(1, fail)]).unwrap_err();
    assert_eq!(
        err,
        ConfigError::Migration {
            from: 1,
            to: 2,
            message: "boom".into()
        }
    );
}

#[test]
fn migrate_missing_step_reports_error() {
    let mut table = toml::Table::new();
    assert!(matches!(
        migrate_with(&mut table, 1, 2, &[]),
        Err(ConfigError::Migration { .. })
    ));
}

fn legacy(text: &str) -> Option<Corners> {
    legacy_corners(&toml::from_str(text).unwrap())
}

#[test]
fn corners_follow_a_cyanotype_theme() {
    let cases = [
        ("theme = \"cyanotype\"", Corners::Square),
        ("theme = \"graphite\"", Corners::Rounded),
        (
            "theme = \"paper\"\ntheme_dark = \"cyanotype\"",
            Corners::Rounded,
        ),
        (
            "theme = \"system\"\ntheme_dark = \"cyanotype\"",
            Corners::Square,
        ),
        (
            "theme = \"system\"\ntheme_light = \"cyanotype\"",
            Corners::Square,
        ),
        ("theme_light = \"cyanotype\"", Corners::Square),
        ("theme = \"system\"", Corners::Rounded),
        ("font_scale = 1.0", Corners::Rounded),
    ];
    for (ui, expected) in cases {
        assert_eq!(legacy(&format!("[ui]\n{ui}\n")), Some(expected), "{ui}");
    }
}

#[test]
fn corners_default_to_rounded_without_a_ui_table() {
    assert_eq!(legacy(""), Some(Corners::Rounded));
    assert_eq!(legacy("ui = 3"), Some(Corners::Rounded));
}

#[test]
fn an_existing_corners_value_is_never_overridden() {
    for value in ["rounded", "square", "bevelled"] {
        let text = format!("[ui]\ntheme = \"cyanotype\"\ncorners = \"{value}\"\n");
        assert_eq!(legacy(&text), None, "{value}");
    }
}
