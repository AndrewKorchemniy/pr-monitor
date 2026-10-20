use std::fs;

use super::*;

#[test]
fn scanner_detects_add_modify_remove() {
    let dir = tempfile::tempdir().unwrap();
    let mut scanner = ThemeDirScanner::new(dir.path().to_path_buf());
    assert!(!scanner.changed(), "nothing yet");

    let file = dir.path().join("mine.toml");
    fs::write(&file, "id = \"mine\"").unwrap();
    assert!(scanner.changed(), "added");
    assert!(!scanner.changed(), "unchanged since");

    fs::write(&file, "id = \"mine\"\nname = \"Mine, longer\"").unwrap();
    assert!(scanner.changed(), "modified");

    fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
    assert!(!scanner.changed(), "non-theme files are ignored");

    fs::remove_file(&file).unwrap();
    assert!(scanner.changed(), "removed");
}

#[test]
fn load_dir_reads_sorted_toml_only() {
    let dir = tempfile::tempdir().unwrap();
    let theme = crate::builtin::builtin_sources()[0].1;
    fs::write(dir.path().join("b.toml"), theme).unwrap();
    fs::write(dir.path().join("a.toml"), theme).unwrap();
    fs::write(dir.path().join("c.txt"), theme).unwrap();

    let (entries, issues) = load_dir(dir.path());

    let ids: Vec<&str> = entries.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(issues, Vec::new());
    assert_eq!(entries[0].1.file.id, "a", "file stem wins");
}
