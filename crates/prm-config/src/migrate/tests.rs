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
