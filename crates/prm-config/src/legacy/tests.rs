use std::path::PathBuf;

use super::*;

struct Dirs {
    _tmp: tempfile::TempDir,
    legacy: PathBuf,
    config: PathBuf,
    local: PathBuf,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    let legacy = base.join("old").join("PRMonitor");
    fs::create_dir_all(&legacy).unwrap();
    Dirs {
        config: base.join("roaming").join("pr-monitor"),
        local: base.join("local").join("pr-monitor"),
        legacy,
        _tmp: tmp,
    }
}

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn run(d: &Dirs) -> LegacyMove {
    move_legacy_folder(&d.legacy, &d.config, &d.local)
}

#[test]
fn legacy_splits_settings_from_local_files_and_removes_the_folder() {
    let d = dirs();
    for rel in [
        "config.toml",
        "keymap.toml",
        "themes/mine.toml",
        "state.json",
        "cache/a.catalog.json",
        "logs/pr-monitor.log",
        "avatars/1.png",
        "icon-256.png",
        "other.txt",
    ] {
        put(&d.legacy, rel, rel);
    }

    let outcome = run(&d);

    for rel in ["config.toml", "keymap.toml", "themes/mine.toml"] {
        assert_eq!(fs::read_to_string(d.config.join(rel)).unwrap(), rel);
    }
    for rel in [
        "state.json",
        "cache/a.catalog.json",
        "logs/pr-monitor.log",
        "avatars/1.png",
        "icon-256.png",
        "other.txt",
    ] {
        assert_eq!(fs::read_to_string(d.local.join(rel)).unwrap(), rel);
    }
    assert!(!d.config.join("state.json").exists());
    assert!(!d.local.join("config.toml").exists());
    assert!(!d.legacy.exists());
    assert!(outcome.found && outcome.removed);
    assert_eq!(outcome.moved.len(), 9);
    assert!(outcome.failed.is_empty() && outcome.skipped.is_empty());
}

#[test]
fn legacy_never_overwrites_an_existing_destination() {
    let d = dirs();
    put(&d.config, "config.toml", "new");
    put(&d.legacy, "config.toml", "old");
    put(&d.legacy, "state.json", "state");

    let outcome = run(&d);

    assert_eq!(
        fs::read_to_string(d.config.join("config.toml")).unwrap(),
        "new"
    );
    assert_eq!(
        fs::read_to_string(d.legacy.join("config.toml")).unwrap(),
        "old"
    );
    assert!(d.local.join("state.json").exists());
    assert_eq!(outcome.skipped, ["config.toml"]);
    assert!(d.legacy.exists() && !outcome.removed);
}

#[test]
fn legacy_is_idempotent_and_a_missing_folder_is_a_no_op() {
    let d = dirs();
    put(&d.legacy, "state.json", "state");
    assert!(run(&d).removed);

    let again = run(&d);

    assert_eq!(again, LegacyMove::default());
    assert_eq!(
        fs::read_to_string(d.local.join("state.json")).unwrap(),
        "state"
    );
}

#[test]
fn legacy_removes_an_empty_folder() {
    let d = dirs();
    let outcome = run(&d);
    assert!(outcome.found && outcome.removed && outcome.moved.is_empty());
    assert!(!d.legacy.exists());
}

#[test]
fn legacy_drops_the_old_instance_lock() {
    let d = dirs();
    put(&d.local, "instance.lock", "new");
    put(&d.legacy, "instance.lock", "old");

    let outcome = run(&d);

    assert_eq!(
        fs::read_to_string(d.local.join("instance.lock")).unwrap(),
        "new"
    );
    assert!(!d.legacy.exists());
    assert!(outcome.skipped.is_empty() && outcome.failed.is_empty());
}

/// `ensure_dirs` makes the empty settings folder on every start, after the move. A move that
/// failed earlier must still retry into it, not be skipped as an existing destination.
#[test]
fn legacy_retry_is_not_blocked_by_an_empty_folder() {
    let d = dirs();
    put(&d.legacy, "themes/mine.toml", "theme");
    fs::create_dir_all(d.config.join("themes")).unwrap();

    let outcome = run(&d);

    assert_eq!(
        fs::read_to_string(d.config.join("themes/mine.toml")).unwrap(),
        "theme"
    );
    assert!(
        outcome.skipped.is_empty() && outcome.failed.is_empty(),
        "{outcome:?}"
    );
    assert!(outcome.removed);
}

/// A destination that cannot be created is reported, not fatal, and leaves the source alone.
#[test]
fn legacy_failure_is_reported_and_keeps_the_source() {
    let d = dirs();
    put(&d.legacy, "state.json", "state");
    put(&d.legacy, "config.toml", "cfg");
    fs::create_dir_all(d.local.parent().unwrap()).unwrap();
    fs::write(&d.local, "a file where the folder should be").unwrap();

    let outcome = run(&d);

    assert_eq!(outcome.failed.len(), 1, "{outcome:?}");
    assert!(outcome.failed[0].starts_with("state.json: "));
    assert!(d.legacy.join("state.json").exists());
    assert!(
        d.config.join("config.toml").exists(),
        "the rest still moves"
    );
    assert!(!outcome.removed);
}

#[test]
fn legacy_copy_fallback_moves_a_tree() {
    let d = dirs();
    put(&d.legacy, "themes/a/b.toml", "deep");
    put(&d.legacy, "themes/c.toml", "top");
    let dest = d.config.join("themes");
    fs::create_dir_all(&d.config).unwrap();

    assert!(relocate_by_copy(&d.legacy.join("themes"), &dest).is_ok());

    assert_eq!(fs::read_to_string(dest.join("a/b.toml")).unwrap(), "deep");
    assert_eq!(fs::read_to_string(dest.join("c.toml")).unwrap(), "top");
    assert!(!d.legacy.join("themes").exists());
}

#[test]
fn legacy_copy_fallback_cleans_up_when_it_fails() {
    let d = dirs();
    put(&d.legacy, "themes/a.toml", "x");
    let dest = d.config.join("themes");
    // why: the parent does not exist, so the copy fails before anything is written.
    assert!(relocate_by_copy(&d.legacy.join("themes"), &dest).is_err());
    assert!(d.legacy.join("themes/a.toml").exists());
    assert!(!dest.exists());
}
