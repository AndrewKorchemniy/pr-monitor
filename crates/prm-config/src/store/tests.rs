use std::{collections::BTreeMap, io};

use prm_domain::testing::{PrBuilder, key, record};

use super::*;
use crate::state::{OrgPersisted, UiState};

const CORRUPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/config_corrupt.toml"
));
const FUTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/config_future.toml"
));

/// A fresh data root in a temp dir (deleted when dropped).
fn temp_paths() -> (tempfile::TempDir, AppPaths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_root(dir.path().to_path_buf());
    paths.ensure_dirs().unwrap();
    (dir, paths)
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn load_missing_creates_defaults_first_run() {
    let (_dir, paths) = temp_paths();
    let load = load_config(&paths);
    assert!(load.first_run);
    assert_eq!(load.config, Config::default());
    assert!(paths.config.exists());
}

#[test]
fn load_corrupt_renames_and_defaults() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.config, CORRUPT).unwrap();
    let load = load_config(&paths);
    assert_eq!(load.config, Config::default());
    assert_eq!(load.issues[0].severity, Severity::Error);
    let moved = load.recovered.unwrap();
    assert!(
        moved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("config.toml.corrupt-")
    );
    assert!(!paths.config.exists());
}

#[test]
fn load_future_schema_flagged_not_saved() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.config, FUTURE).unwrap();
    let load = load_config(&paths);
    assert!(load.newer_schema);
    assert_eq!(load.config.refresh.interval_minutes, 7);
    assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), FUTURE);
}

#[test]
fn atomic_write_replaces_no_temp_left() {
    let (_dir, paths) = temp_paths();
    let target = paths.root.join("file.txt");
    atomic_write(&target, b"one").unwrap();
    atomic_write(&target, b"two").unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "two");
    let names = files_in(&paths.root);
    assert!(names.iter().all(|n| !n.starts_with(".tmp")), "{names:?}");
}

#[test]
fn atomic_write_retries_permission_denied() {
    let (_dir, paths) = temp_paths();
    let target = paths.root.join("file.txt");
    let mut attempts = 0;
    atomic_write_with(&target, b"data", |tmp, path| {
        attempts += 1;
        if attempts < 3 {
            let error = io::Error::from(io::ErrorKind::PermissionDenied);
            return Err(tempfile::PersistError { error, file: tmp });
        }
        tmp.persist(path).map(|_| ())
    })
    .unwrap();
    assert_eq!(attempts, 3);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "data");
}

#[test]
fn atomic_write_gives_up_after_retries() {
    let (_dir, paths) = temp_paths();
    let result = atomic_write_with(&paths.root.join("f"), b"x", |tmp, _| {
        let error = io::Error::from(io::ErrorKind::PermissionDenied);
        Err(tempfile::PersistError { error, file: tmp })
    });
    assert!(matches!(
        result,
        Err(ConfigError::Io {
            kind: io::ErrorKind::PermissionDenied,
            ..
        })
    ));
}

fn sample_state() -> PersistedState {
    let rec = record(PrBuilder::new(key("contoso", 7, 22)).build(), vec![]);
    let org = OrgPersisted {
        baseline: true,
        records: BTreeMap::from([(rec.pr.key.clone(), rec)]),
        ..OrgPersisted::default()
    };
    PersistedState {
        orgs: BTreeMap::from([(prm_domain::testing::org("contoso"), org)]),
        ui: UiState {
            peek_open: true,
            ..UiState::default()
        },
        ..PersistedState::default()
    }
}

#[test]
fn state_roundtrip_snapshot() {
    let (_dir, paths) = temp_paths();
    let state = sample_state();
    FileRepo::new(paths.clone()).save_state(&state).unwrap();
    let loaded = load_state(&paths);
    assert_eq!(loaded.state, state);
    assert_eq!(loaded.issue, None);
    insta::assert_json_snapshot!(state);
}

#[test]
fn state_prkey_keys_are_strings() {
    let json = serde_json::to_value(sample_state()).unwrap();
    let records = &json["orgs"]["contoso"]["records"];
    assert!(
        records
            .get("contoso/00000000-0000-0000-0000-000000000007/22")
            .is_some()
    );
}

#[test]
fn state_corrupt_backed_up_and_defaulted() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.state, "{ not json").unwrap();
    let loaded = load_state(&paths);
    assert_eq!(loaded.state, PersistedState::default());
    assert!(loaded.issue.is_some());
    assert!(
        files_in(&paths.root)
            .iter()
            .any(|n| n.starts_with("state.json.corrupt-"))
    );
}

#[test]
fn catalogs_load_best_effort() {
    let (_dir, paths) = temp_paths();
    let org = prm_domain::testing::org("contoso");
    let catalog = Catalog {
        fetched_at: prm_domain::testing::ts(1),
        projects: vec![],
        repos: vec![],
    };
    FileRepo::new(paths.clone())
        .save_catalog(&org, &catalog)
        .unwrap();
    std::fs::write(
        paths.catalog_file(&prm_domain::testing::org("broken")),
        "nope",
    )
    .unwrap();
    let loaded = load_catalogs(&paths, &[org.clone(), prm_domain::testing::org("broken")]);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[&org], catalog);
}

#[test]
fn memory_repo_records_and_fails_next() {
    let repo = MemoryRepo::default();
    repo.fail_next(ConfigError::NoDataDir);
    assert_eq!(
        repo.save_state(&PersistedState::default()),
        Err(ConfigError::NoDataDir)
    );
    repo.save_state(&PersistedState::default()).unwrap();
    assert_eq!(repo.last_state(), Some(PersistedState::default()));
}
