use std::{ffi::OsString, path::Path};

use super::*;

const MOUNT: &str = "/tmp/.mount_PRMonixyz";
const MOUNTED: &str = "/tmp/.mount_PRMonixyz/usr/bin/pr-monitor";
const IMAGE: &str = "/home/me/Apps/PR Monitor.AppImage";

fn os(value: &str) -> OsString {
    OsString::from(value)
}

#[test]
fn pick_keeps_the_exe_outside_an_appimage() {
    assert_eq!(pick(Path::new(MOUNTED), None, None), Path::new(MOUNTED));
}

#[test]
fn pick_prefers_the_appimage_file_when_running_from_its_mount() {
    assert_eq!(
        pick(Path::new(MOUNTED), Some(os(IMAGE)), Some(os(MOUNT))),
        Path::new(IMAGE)
    );
}

#[test]
fn pick_ignores_variables_inherited_from_another_appimage() {
    let exe = Path::new("/usr/local/bin/pr-monitor");
    assert_eq!(pick(exe, Some(os(IMAGE)), Some(os(MOUNT))), exe);
    assert_eq!(pick(exe, Some(os(IMAGE)), None), exe);
}

#[test]
fn pick_ignores_empty_variables() {
    let exe = Path::new(MOUNTED);
    assert_eq!(pick(exe, Some(os("")), Some(os(MOUNT))), exe);
    assert_eq!(pick(exe, Some(os(IMAGE)), Some(os(""))), exe);
}

#[test]
fn exec_field_leaves_a_plain_path_alone() {
    assert_eq!(
        exec_field(Path::new("/opt/pr-monitor/pr-monitor")),
        "/opt/pr-monitor/pr-monitor"
    );
}

#[test]
fn exec_field_quotes_a_path_with_spaces() {
    assert_eq!(
        exec_field(Path::new(IMAGE)),
        "\"/home/me/Apps/PR Monitor.AppImage\""
    );
}

#[test]
fn exec_field_escapes_quote_dollar_backslash_and_percent() {
    assert_eq!(
        exec_field(Path::new("/a b/\"$\\%")),
        "\"/a b/\\\\\"\\\\$\\\\\\\\%%\""
    );
}
