//! Where PR Monitor keeps its files. The root resolves `--home` > `PRMONITOR_HOME` >
//! `%LOCALAPPDATA%\PRMonitor` (or the platform equivalent).

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use prm_domain::{OrgId, app::DATA_DIR_NAME};

use crate::error::ConfigError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub themes: PathBuf,
    pub keymap: PathBuf,
    pub icon_png: PathBuf,
}

impl AppPaths {
    pub fn from_root(root: PathBuf) -> Self {
        Self {
            config: root.join("config.toml"),
            state: root.join("state.json"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            themes: root.join("themes"),
            keymap: root.join("keymap.toml"),
            icon_png: root.join("icon-256.png"),
            root,
        }
    }

    pub fn ensure_dirs(&self) -> Result<(), ConfigError> {
        for dir in [&self.root, &self.cache, &self.logs, &self.themes] {
            std::fs::create_dir_all(dir).map_err(|e| ConfigError::io(dir, &e))?;
        }
        Ok(())
    }

    /// `cache/<org>.catalog.json`.
    pub fn catalog_file(&self, org: &OrgId) -> PathBuf {
        self.cache.join(format!("{org}.catalog.json"))
    }
}

/// Pure: the caller passes the CLI value, the env value and the platform data dir. An empty
/// env value is ignored; relative paths are made absolute against `cwd`.
pub fn resolve_root(
    cli: Option<PathBuf>,
    env: Option<OsString>,
    data_local: Option<PathBuf>,
    cwd: &Path,
) -> Result<PathBuf, ConfigError> {
    let explicit = cli.or_else(|| env.filter(|v| !v.is_empty()).map(PathBuf::from));
    let root = match explicit {
        Some(path) => path,
        None => data_local
            .ok_or(ConfigError::NoDataDir)?
            .join(DATA_DIR_NAME),
    };
    Ok(if root.is_absolute() {
        root
    } else {
        cwd.join(root)
    })
}

#[cfg(test)]
mod tests;
