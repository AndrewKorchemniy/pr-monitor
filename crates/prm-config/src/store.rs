//! Loading and saving `config.toml`, `state.json` and catalog caches. Loads never fail: the
//! worst case is defaults plus an issue. Writes are atomic (temp file, then rename).

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use prm_domain::{OrgId, model::Catalog};

use crate::{
    error::{ConfigError, ConfigIssue, Severity},
    migrate::{CURRENT_SCHEMA, MIGRATIONS, migrate_with},
    paths::AppPaths,
    schema::Config,
    state::{PersistedState, STATE_SCHEMA},
    validate::normalize,
};

/// Waits between attempts when antivirus or the indexer briefly holds the target file.
const PERSIST_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
];

/// Persistence port used by the service, so tests never touch the disk.
pub trait StateRepo: Send + Sync + 'static {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError>;
    fn save_state(&self, state: &PersistedState) -> Result<(), ConfigError>;
    fn save_catalog(&self, org: &OrgId, catalog: &Catalog) -> Result<(), ConfigError>;
    /// Removes the org's catalog file; its state lives in memory and is saved separately.
    fn delete_org_data(&self, org: &OrgId) -> Result<(), ConfigError>;
}

#[derive(Clone, Debug)]
pub struct FileRepo {
    paths: AppPaths,
}

impl FileRepo {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }
}

impl StateRepo for FileRepo {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError> {
        save_config(&self.paths, cfg)
    }

    fn save_state(&self, state: &PersistedState) -> Result<(), ConfigError> {
        let json =
            serde_json::to_vec(state).map_err(|e| ConfigError::Serialize(e.to_string().into()))?;
        atomic_write(&self.paths.state, &json)
    }

    fn save_catalog(&self, org: &OrgId, catalog: &Catalog) -> Result<(), ConfigError> {
        let json = serde_json::to_vec(catalog)
            .map_err(|e| ConfigError::Serialize(e.to_string().into()))?;
        atomic_write(&self.paths.catalog_file(org), &json)
    }

    fn delete_org_data(&self, org: &OrgId) -> Result<(), ConfigError> {
        let path = self.paths.catalog_file(org);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ConfigError::io(&path, &e)),
        }
    }
}

pub fn save_config(paths: &AppPaths, cfg: &Config) -> Result<(), ConfigError> {
    let text =
        toml::to_string_pretty(cfg).map_err(|e| ConfigError::Serialize(e.to_string().into()))?;
    atomic_write(&paths.config, text.as_bytes())
}

/// The result of loading `config.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLoad {
    pub config: Config,
    pub issues: Vec<ConfigIssue>,
    /// No config file existed.
    pub first_run: bool,
    pub migrated_from: Option<u32>,
    /// Written by a newer version: loaded best-effort and never auto-saved.
    pub newer_schema: bool,
    /// Where an unreadable file was moved.
    pub recovered: Option<PathBuf>,
}

pub fn load_config(paths: &AppPaths) -> ConfigLoad {
    let mut load = ConfigLoad {
        config: Config::default(),
        issues: Vec::new(),
        first_run: false,
        migrated_from: None,
        newer_schema: false,
        recovered: None,
    };
    let text = match std::fs::read_to_string(&paths.config) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            load.first_run = true;
            if let Err(err) = save_config(paths, &load.config) {
                tracing::warn!(error = %err, "could not write the default config");
            }
            return load;
        }
        Err(e) => return recover(paths, load, &e.to_string()),
    };
    match parse_config(paths, &text, &mut load) {
        Ok(config) => {
            let (config, issues) = normalize(config);
            load.config = config;
            load.issues.extend(issues);
            load
        }
        Err(message) => recover(paths, load, &message),
    }
}

/// Parses, migrates if older, and deserializes. Records version facts on `load`.
fn parse_config(paths: &AppPaths, text: &str, load: &mut ConfigLoad) -> Result<Config, String> {
    let mut table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let version = table
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(CURRENT_SCHEMA);
    if version > CURRENT_SCHEMA {
        load.newer_schema = true;
        load.issues.push(ConfigIssue::new(
            Severity::Warning,
            "schema_version",
            "This config was written by a newer PR Monitor; some settings may be ignored.",
        ));
    } else if version < CURRENT_SCHEMA {
        migrate_with(&mut table, version, CURRENT_SCHEMA, MIGRATIONS).map_err(|e| e.to_string())?;
        backup(
            &paths.config,
            &paths.root.join(format!("config.v{version}.bak")),
        );
        load.migrated_from = Some(version);
    }
    let reserialized = toml::to_string(&table).map_err(|e| e.to_string())?;
    let config: Config = toml::from_str(&reserialized).map_err(|e| e.to_string())?;
    if load.migrated_from.is_some()
        && let Err(e) = save_config(paths, &config)
    {
        tracing::warn!(error = %e, "could not save the migrated config");
    }
    Ok(config)
}

/// Moves an unreadable file aside and continues with defaults.
fn recover(paths: &AppPaths, mut load: ConfigLoad, reason: &str) -> ConfigLoad {
    let moved = paths
        .root
        .join(format!("config.toml.corrupt-{}", unix_now()));
    match std::fs::rename(&paths.config, &moved) {
        Ok(()) => load.recovered = Some(moved.clone()),
        Err(e) => tracing::warn!(error = %e, "could not move the unreadable config"),
    }
    tracing::error!(reason, "config could not be read; using defaults");
    load.issues.push(ConfigIssue::new(
        Severity::Error,
        "config.toml",
        format!(
            "Your config could not be read and was moved to {}.",
            moved.display()
        ),
    ));
    load
}

fn backup(from: &Path, to: &Path) {
    if let Err(e) = std::fs::copy(from, to) {
        tracing::warn!(error = %e, to = %to.display(), "config backup failed");
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The result of loading `state.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateLoad {
    pub state: PersistedState,
    pub issue: Option<ConfigIssue>,
}

/// Missing: defaults. Corrupt or newer: backed up to `state.json.corrupt-<unix>`, then defaults.
pub fn load_state(paths: &AppPaths) -> StateLoad {
    let Ok(text) = std::fs::read_to_string(&paths.state) else {
        return StateLoad {
            state: PersistedState::default(),
            issue: None,
        };
    };
    match serde_json::from_str::<PersistedState>(&text) {
        Ok(state) if state.schema_version <= STATE_SCHEMA => StateLoad { state, issue: None },
        Ok(_) => state_reset(paths, "it was written by a newer PR Monitor"),
        Err(e) => state_reset(paths, &e.to_string()),
    }
}

fn state_reset(paths: &AppPaths, reason: &str) -> StateLoad {
    let moved = paths
        .root
        .join(format!("state.json.corrupt-{}", unix_now()));
    if let Err(e) = std::fs::rename(&paths.state, &moved) {
        tracing::warn!(error = %e, "could not move the unreadable state");
    }
    tracing::warn!(reason, "state reset");
    StateLoad {
        state: PersistedState::default(),
        issue: Some(ConfigIssue::new(
            Severity::Info,
            "state.json",
            "Cached pull requests were reset; they reload on the next refresh.",
        )),
    }
}

/// Cached catalogs for the given orgs; unreadable files are skipped.
pub fn load_catalogs(paths: &AppPaths, orgs: &[OrgId]) -> HashMap<OrgId, Catalog> {
    orgs.iter()
        .filter_map(|org| {
            let text = std::fs::read_to_string(paths.catalog_file(org)).ok()?;
            let catalog = serde_json::from_str(&text).ok()?;
            Some((org.clone(), catalog))
        })
        .collect()
}

/// Writes `bytes` to a temp file next to `path`, syncs it, then renames it over `path`.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    atomic_write_with(path, bytes, |tmp, target| tmp.persist(target).map(|_| ()))
}

/// [`atomic_write`] with an injectable `persist`, so retries are testable.
pub fn atomic_write_with<F>(path: &Path, bytes: &[u8], mut persist: F) -> Result<(), ConfigError>
where
    F: FnMut(tempfile::NamedTempFile, &Path) -> Result<(), tempfile::PersistError>,
{
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| ConfigError::io(dir, &e))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| ConfigError::io(dir, &e))?;
    tmp.write_all(bytes)
        .map_err(|e| ConfigError::io(path, &e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| ConfigError::io(path, &e))?;
    let mut delays = PERSIST_RETRY_DELAYS.iter();
    loop {
        match persist(tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) if e.error.kind() == std::io::ErrorKind::PermissionDenied => {
                let Some(delay) = delays.next() else {
                    return Err(ConfigError::io(path, &e.error));
                };
                std::thread::sleep(*delay);
                tmp = e.file;
            }
            Err(e) => return Err(ConfigError::io(path, &e.error)),
        }
    }
}

/// An in-memory [`StateRepo`] for tests: records saves and can fail the next one.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
pub struct MemoryRepo {
    saved: std::sync::Mutex<Vec<Saved>>,
    fail_next: std::sync::Mutex<Option<ConfigError>>,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Debug, PartialEq)]
pub enum Saved {
    Config(Box<Config>),
    State(Box<PersistedState>),
    Catalog(OrgId),
    Deleted(OrgId),
}

#[cfg(any(test, feature = "test-support"))]
impl MemoryRepo {
    pub fn saved(&self) -> Vec<Saved> {
        self.saved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn last_state(&self) -> Option<PersistedState> {
        self.saved().into_iter().rev().find_map(|s| match s {
            Saved::State(state) => Some(*state),
            Saved::Config(_) | Saved::Catalog(_) | Saved::Deleted(_) => None,
        })
    }

    pub fn fail_next(&self, error: ConfigError) {
        *self
            .fail_next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
    }

    fn record(&self, saved: Saved) -> Result<(), ConfigError> {
        let failure = self
            .fail_next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(error) = failure {
            return Err(error);
        }
        self.saved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(saved);
        Ok(())
    }
}

#[cfg(any(test, feature = "test-support"))]
impl StateRepo for MemoryRepo {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError> {
        self.record(Saved::Config(Box::new(cfg.clone())))
    }

    fn save_state(&self, state: &PersistedState) -> Result<(), ConfigError> {
        self.record(Saved::State(Box::new(state.clone())))
    }

    fn save_catalog(&self, org: &OrgId, _catalog: &Catalog) -> Result<(), ConfigError> {
        self.record(Saved::Catalog(org.clone()))
    }

    fn delete_org_data(&self, org: &OrgId) -> Result<(), ConfigError> {
        self.record(Saved::Deleted(org.clone()))
    }
}

#[cfg(test)]
mod tests;
