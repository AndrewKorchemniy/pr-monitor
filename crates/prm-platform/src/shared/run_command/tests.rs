use super::*;

fn exe() -> PathBuf {
    PathBuf::from(r"C:\Program Files\PR Monitor\pr-monitor.exe")
}

#[test]
fn run_command_quotes_exe() {
    assert_eq!(
        format_run_command(&exe(), None).unwrap(),
        r#""C:\Program Files\PR Monitor\pr-monitor.exe" --minimized"#
    );
}

#[test]
fn run_command_with_home() {
    let home = PathBuf::from(r"D:\prm data");
    assert_eq!(
        format_run_command(&exe(), Some(&home)).unwrap(),
        r#""C:\Program Files\PR Monitor\pr-monitor.exe" --minimized --home "D:\prm data""#
    );
}

#[test]
fn run_command_rejects_quote_in_path() {
    let bad = PathBuf::from("C:\\a\"b\\pr-monitor.exe");
    let error = format_run_command(&bad, None).unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Denied);
    assert!(protocol_command(&bad).is_err());
}

#[test]
fn parse_run_command_quoted_and_bare() {
    assert_eq!(
        parse_run_command(r#""C:\Program Files\PR Monitor\pr-monitor.exe" --minimized"#),
        Some(exe())
    );
    assert_eq!(
        parse_run_command(r"C:\tools\pr-monitor.exe --minimized"),
        Some(PathBuf::from(r"C:\tools\pr-monitor.exe"))
    );
    assert_eq!(parse_run_command("   "), None);
    assert_eq!(parse_run_command("\"\""), None);
}

#[test]
fn startup_approved_table() {
    let cases: [(Option<&[u8]>, bool); 6] = [
        (None, true),
        (Some(&[]), true),
        (Some(&[0x02, 0, 0]), true),
        (Some(&[0x03, 0, 0]), false),
        (Some(&[0x06, 0, 0]), true),
        (Some(&[0x07, 0, 0]), false),
    ];
    for (bytes, enabled) in cases {
        assert_eq!(startup_approved_enabled(bytes), enabled, "{bytes:?}");
    }
    assert!(startup_approved_enabled(Some(&STARTUP_APPROVED_ENABLED)));
}

#[test]
fn protocol_command_format() {
    assert_eq!(
        protocol_command(&exe()).unwrap(),
        r#""C:\Program Files\PR Monitor\pr-monitor.exe" --activate "%1""#
    );
}

#[test]
fn run_value_name_default_and_isolated() {
    assert_eq!(
        run_value_name(InstanceScope::Default, "abcd1234"),
        "PRMonitor"
    );
    assert_eq!(
        run_value_name(InstanceScope::Isolated, "abcd1234"),
        "PRMonitor-abcd1234"
    );
}
