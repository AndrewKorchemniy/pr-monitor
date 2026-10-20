use super::*;

#[test]
fn crash_reports_name_the_build_place_and_image_base() {
    let report = crash_report(&CrashInfo {
        message: "index out of bounds",
        location: Some("src/view.rs:10:5".to_owned()),
        thread: "main",
        image_base: Some(0x4000_0000),
        backtrace: "0: frame",
    });
    for part in [
        "index out of bounds",
        "src/view.rs:10:5",
        "thread=main",
        "image_base=0x40000000",
        build_info::VERSION,
        "0: frame",
    ] {
        assert!(report.contains(part), "{part} missing from {report}");
    }
}

#[test]
fn unknown_details_are_spelled_out() {
    let report = crash_report(&CrashInfo {
        message: "boom",
        location: None,
        thread: "svc",
        image_base: None,
        backtrace: "",
    });
    assert!(report.contains("location=unknown"));
    assert!(report.contains("image_base=unknown"));
}

#[test]
fn pruning_keeps_only_the_newest_crash_logs() {
    let dir = tempfile::tempdir().expect("temp dir");
    for i in 0..4 {
        std::fs::write(dir.path().join(format!("crash-{i}-1.log")), "x").expect("write");
    }
    std::fs::write(dir.path().join("pr-monitor.log"), "keep").expect("write");
    prune_crash_logs(dir.path(), 2);
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.iter().filter(|n| n.starts_with("crash-")).count(), 2);
    assert!(
        names.contains(&"pr-monitor.log".to_owned()),
        "other files are untouched"
    );
}
