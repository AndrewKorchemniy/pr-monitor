use super::*;

#[test]
fn os_description_win11_from_build() {
    let facts = OsFacts {
        product: Some("Windows 10 Enterprise"),
        display_version: Some("24H2"),
        build: Some(26_100),
        ubr: Some(2_033),
        arch: "x86_64",
    };
    assert_eq!(
        format_os_description(&facts),
        "Windows 11 Enterprise 24H2 (26100.2033) x86_64"
    );
}

#[test]
fn os_description_keeps_windows_10() {
    let facts = OsFacts {
        product: Some("Windows 10 Pro"),
        display_version: Some("22H2"),
        build: Some(19_045),
        ubr: None,
        arch: "x86_64",
    };
    assert_eq!(
        format_os_description(&facts),
        "Windows 10 Pro 22H2 (19045) x86_64"
    );
}

#[test]
fn os_description_missing_values() {
    assert_eq!(format_os_description(&OsFacts::default()), "Windows");
    let arch_only = OsFacts {
        arch: "aarch64",
        ..OsFacts::default()
    };
    assert_eq!(format_os_description(&arch_only), "Windows aarch64");
}
