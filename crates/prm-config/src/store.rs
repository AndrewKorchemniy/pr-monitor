//! Loading and saving `config.toml`, `state.json` and catalog caches. Loads never fail: the
//! worst case is defaults plus an issue. Writes are atomic (temp file, then rename).

use std::{
    collections::HashMap,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use prm_domain::{OrgId, app::APP_DISPLAY_NAME, model::Catalog};

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

/// Windows `ERROR_SHARING_VIOLATION` (32) and `ERROR_LOCK_VIOLATION` (33).
// why: antivirus, the indexer or a sync client holding the file open fails reads and renames
// with these codes, which std reports under an uncategorized error kind, so match the code.
const WINDOWS_SHARING_ERRORS: [i32; 2] = [32, 33];

/// Persistence port used by the service, so tests never touch the disk.
pub trait StateRepo: Send + Sync + 'static {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError>;
    fn save_state(&self, state: &PersistedState) -> Result<(), ConfigError>;
    fn save_catalog(&self, org: &OrgId, catalog: &Catalog) -> Result<(), ConfigError>;
    /// Removes the org's catalog file; its state lives in memory and is saved separately.
    fn delete_org_data(&self, org: &OrgId) -> Result<(), ConfigError>;
}

/// The on-disk [`StateRepo`], rooted at the app's data directory.
#[derive(Clone, Debug)]
pub struct FileRepo {
    paths: AppPaths,
    /// Refuse to overwrite `config.toml` (see [`ConfigLoad::read_only`]).
    config_read_only: bool,
}

impl FileRepo {
    /// A repo that may write every file.
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            config_read_only: false,
        }
    }

    /// A repo that keeps `config.toml` untouched when the load says it must not be saved.
    pub fn from_load(paths: AppPaths, load: &ConfigLoad) -> Self {
        Self {
            paths,
            config_read_only: load.read_only,
        }
    }
}

impl StateRepo for FileRepo {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError> {
        if self.config_read_only {
            return Err(ConfigError::ReadOnly);
        }
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
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ConfigError::io(&path, &e)),
        }
    }
}

pub fn save_config(paths: &AppPaths, cfg: &Config) -> Result<(), ConfigError> {
    let text =
        toml::to_string_pretty(cfg).map_err(|e| ConfigError::Serialize(e.to_string().into()))?;
    atomic_write(&paths.config, text.as_bytes())
}

/// The result of loading `config.toml`. Recovery and newer-schema events reach the user
/// through `issues`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLoad {
    pub config: Config,
    pub issues: Vec<ConfigIssue>,
    /// `config.toml` must not be overwritten this session: a newer version wrote it (saving
    /// would drop the keys this version doesn't know), or it could not be read and was left
    /// in place. Build the repo with [`FileRepo::from_load`] to enforce this.
    pub read_only: bool,
}

/// Whether a parsed config still has to be written back in the current schema.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Origin {
    Current,
    Migrated,
}

pub fn load_config(paths: &AppPaths) -> ConfigLoad {
    let mut load = ConfigLoad {
        config: Config::default(),
        issues: Vec::new(),
        read_only: false,
    };
    let text = match read_text(&paths.config) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Err(err) = save_config(paths, &load.config) {
                tracing::warn!(error = %err, "could not write the default config");
            }
            return load;
        }
        Err(e) => return recover(paths, load, &e.to_string()),
    };
    match parse_config(paths, &text, &mut load) {
        Ok((config, origin)) => {
            let (config, issues) = normalize(config);
            if origin == Origin::Migrated
                && let Err(e) = save_config(paths, &config)
            {
                tracing::warn!(error = %e, "could not save the migrated config");
            }
            load.config = config;
            load.issues.extend(issues);
            load
        }
        Err(message) => recover(paths, load, &message),
    }
}

/// Parses and, if older, migrates. A newer file loads best-effort and is marked read-only.
fn parse_config(
    paths: &AppPaths,
    text: &str,
    load: &mut ConfigLoad,
) -> Result<(Config, Origin), String> {
    let mut table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let version = table
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(CURRENT_SCHEMA);
    if version < CURRENT_SCHEMA {
        migrate_with(&mut table, version, CURRENT_SCHEMA, MIGRATIONS).map_err(|e| e.to_string())?;
        backup(
            &paths.config,
            &paths.root.join(format!("config.v{version}.bak")),
        );
        let config = table.try_into().map_err(|e| e.to_string())?;
        return Ok((config, Origin::Migrated));
    }
    if version > CURRENT_SCHEMA {
        load.read_only = true;
        load.issues.push(ConfigIssue::new(
            Severity::Warning,
            "schema_version",
            format!(
                "This config was written by a newer {APP_DISPLAY_NAME}; some settings may be \
                 ignored and changes won't be saved."
            ),
        ));
    }
    // Straight from the user's text, so error positions point into their file.
    let config = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok((config, Origin::Current))
}

/// Continues with defaults after `config.toml` could not be read or parsed. The file is moved
/// aside when possible; otherwise it stays where it is and is protected from saves.
fn recover(paths: &AppPaths, mut load: ConfigLoad, reason: &str) -> ConfigLoad {
    tracing::error!(reason, "config could not be read; using defaults");
    let message = if let Some(moved) = quarantine(&paths.config, &paths.root, "config.toml") {
        format!(
            "Your config could not be read and was moved to {}.",
            moved.display()
        )
    } else {
        load.read_only = true;
        "Your config could not be read; using defaults. The file was left in place and won't \
         be overwritten."
            .to_owned()
    };
    load.issues
        .push(ConfigIssue::new(Severity::Error, "config.toml", message));
    load
}

/// Renames an unreadable file to `<root>/<name>.corrupt-<unix>`. Returns the new path, or
/// `None` when the file could not be moved.
fn quarantine(path: &Path, root: &Path, name: &str) -> Option<PathBuf> {
    let moved = root.join(format!("{name}.corrupt-{}", unix_now()));
    match std::fs::rename(path, &moved) {
        Ok(()) => Some(moved),
        Err(e) => {
            tracing::warn!(error = %e, file = name, "could not move the unreadable file aside");
            None
        }
    }
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

/// Reads a UTF-8 file, retrying briefly while another process holds it open.
fn read_text(path: &Path) -> io::Result<String> {
    let mut delays = PERSIST_RETRY_DELAYS.iter();
    loop {
        match std::fs::read_to_string(path) {
            Err(e) if is_sharing_violation(&e) => {
                let Some(delay) = delays.next() else {
                    return Err(e);
                };
                std::thread::sleep(*delay);
            }
            result => return result,
        }
    }
}

fn is_sharing_violation(error: &io::Error) -> bool {
    cfg!(windows)
        && error
            .raw_os_error()
            .is_some_and(|code| WINDOWS_SHARING_ERRORS.contains(&code))
}

/// The result of loading `state.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateLoad {
    pub state: PersistedState,
    pub issue: Option<ConfigIssue>,
}

/// Missing: defaults. Unreadable, corrupt or newer: moved to `state.json.corrupt-<unix>` when
/// possible, then defaults and an issue.
pub fn load_state(paths: &AppPaths) -> StateLoad {
    let text = match read_text(&paths.state) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return StateLoad {
                state: PersistedState::default(),
                issue: None,
            };
        }
        Err(e) => return state_reset(paths, &e.to_string()),
    };
    match serde_json::from_str::<PersistedState>(&text) {
        Ok(state) if state.schema_version <= STATE_SCHEMA => StateLoad { state, issue: None },
        Ok(_) => state_reset(
            paths,
            &format!("it was written by a newer {APP_DISPLAY_NAME}"),
        ),
        Err(e) => state_reset(paths, &e.to_string()),
    }
}

fn state_reset(paths: &AppPaths, reason: &str) -> StateLoad {
    quarantine(&paths.state, &paths.root, "state.json");
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
pub(crate) fn atomic_write_with<F>(
    path: &Path,
    bytes: &[u8],
    mut persist: F,
) -> Result<(), ConfigError>
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
            Err(e)
                if e.error.kind() == io::ErrorKind::PermissionDenied
                    || is_sharing_violation(&e.error) =>
            {
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

#[cfg(test)]
mod tests;
