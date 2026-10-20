use std::{collections::BTreeMap, io};

use prm_domain::testing::{PrBuilder, key, record};

use super::*;
use crate::{
    state::{OrgPersisted, UiState},
    testing::MemoryRepo,
};

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
fn load_missing_creates_defaults() {
    let (_dir, paths) = temp_paths();
    let load = load_config(&paths);
    assert_eq!(load.config, Config::default());
    assert_eq!(load.issues, Vec::new());
    assert!(!load.read_only);
    assert!(paths.config.exists());
}

#[test]
fn load_corrupt_renames_and_defaults() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.config, CORRUPT).unwrap();
    let load = load_config(&paths);
    assert_eq!(load.config, Config::default());
    assert_eq!(load.issues[0].severity, Severity::Error);
    assert!(load.issues[0].message.contains("was moved to"));
    assert!(!load.read_only);
    assert!(
        files_in(&paths.root)
            .iter()
            .any(|n| n.starts_with("config.toml.corrupt-"))
    );
    assert!(!paths.config.exists());
}

/// A read error used to report "moved to …" even when the move failed, then let the next
/// settings save overwrite the real file with defaults.
#[test]
fn recover_unmovable_config_is_left_in_place_and_read_only() {
    let (_dir, paths) = temp_paths();
    let empty = ConfigLoad {
        config: Config::default(),
        issues: Vec::new(),
        read_only: false,
    };
    // Nothing exists at the config path, so the move fails, as it does for a locked file.
    let load = recover(&paths, empty, "locked");
    assert!(load.read_only);
    assert_eq!(load.issues[0].severity, Severity::Error);
    assert!(
        load.issues[0].message.contains("left in place"),
        "{}",
        load.issues[0].message
    );
}

#[test]
fn load_future_schema_is_read_only() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.config, FUTURE).unwrap();
    let load = load_config(&paths);
    assert!(load.read_only);
    assert_eq!(load.config.refresh.interval_minutes, 7);
    assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), FUTURE);
}

/// A config from a newer version used to be saved back on the first settings change,
/// dropping the keys this version doesn't know while keeping the newer schema number.
#[test]
fn file_repo_from_newer_schema_load_refuses_to_save_config() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.config, FUTURE).unwrap();
    let load = load_config(&paths);
    let repo = FileRepo::from_load(paths.clone(), &load);
    assert_eq!(repo.save_config(&load.config), Err(ConfigError::ReadOnly));
    assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), FUTURE);
    repo.save_state(&PersistedState::default()).unwrap();
}

#[test]
fn file_repo_from_normal_load_saves_config() {
    let (_dir, paths) = temp_paths();
    let load = load_config(&paths);
    let mut config = load.config.clone();
    config.refresh.interval_minutes = 9;
    FileRepo::from_load(paths.clone(), &load)
        .save_config(&config)
        .unwrap();
    assert_eq!(load_config(&paths).config.refresh.interval_minutes, 9);
}

/// One number too wide for its type used to fail the whole file, which was then moved aside.
#[test]
fn load_oversized_number_clamps_instead_of_recovering() {
    let (_dir, paths) = temp_paths();
    let text = "[network]\nmax_concurrency = 300\n";
    std::fs::write(&paths.config, text).unwrap();
    let load = load_config(&paths);
    assert_eq!(
        load.config.network.max_concurrency,
        *crate::limits::MAX_CONCURRENCY.end()
    );
    let paths_with_issues: Vec<&str> = load.issues.iter().map(|i| &*i.path).collect();
    assert_eq!(paths_with_issues, vec!["network.max_concurrency"]);
    assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), text);
}

#[test]
fn load_parse_error_points_into_the_users_file() {
    let (_dir, paths) = temp_paths();
    let text = "[refresh]\njitter_percent = 5\n\n\ninterval_minutes = \"five\"\n";
    std::fs::write(&paths.config, text).unwrap();
    let mut load = ConfigLoad {
        config: Config::default(),
        issues: Vec::new(),
        read_only: false,
    };
    let error = parse_config(&paths, text, &mut load).unwrap_err();
    assert!(error.contains("line 5"), "{error}");
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

/// A Windows sharing violation (antivirus scanning the target) has an uncategorized error
/// kind, so it used to fail the save at once instead of retrying.
#[cfg(windows)]
#[test]
fn atomic_write_retries_sharing_violation() {
    let (_dir, paths) = temp_paths();
    let target = paths.root.join("file.txt");
    let mut attempts = 0;
    atomic_write_with(&target, b"data", |tmp, path| {
        attempts += 1;
        if attempts < 2 {
            let error = io::Error::from_raw_os_error(WINDOWS_SHARING_ERRORS[0]);
            return Err(tempfile::PersistError { error, file: tmp });
        }
        tmp.persist(path).map(|_| ())
    })
    .unwrap();
    assert_eq!(attempts, 2);
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

/// Any read error used to count as "missing": defaults with no issue and no backup, after
/// which the next save replaced the user's seen map, records and sign-out flags.
#[test]
fn state_unreadable_bytes_backed_up_with_issue() {
    let (_dir, paths) = temp_paths();
    std::fs::write(&paths.state, [0xff, 0xfe, 0x00, 0x7b]).unwrap();
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
fn state_read_error_reports_an_issue() {
    let (_dir, paths) = temp_paths();
    std::fs::create_dir(&paths.state).unwrap();
    let loaded = load_state(&paths);
    assert_eq!(loaded.state, PersistedState::default());
    assert!(loaded.issue.is_some());
}

#[test]
fn state_missing_is_silent() {
    let (_dir, paths) = temp_paths();
    assert_eq!(
        load_state(&paths),
        StateLoad {
            state: PersistedState::default(),
            issue: None
        }
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
