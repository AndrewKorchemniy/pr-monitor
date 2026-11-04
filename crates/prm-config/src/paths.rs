//! Where PR Monitor keeps its files, in two folders with the same name. Settings (`config.toml`,
//! `keymap.toml`, `themes/`) live under the platform config dir (`%APPDATA%\pr-monitor`); the
//! rest (state, cache, logs, avatars, instance lock) under the local data dir
//! (`%LOCALAPPDATA%\pr-monitor`, also the Velopack install folder). `--home` > `PRMONITOR_HOME`
//! names one folder that holds everything.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use prm_domain::{OrgId, app::DATA_DIR_NAME};

use crate::error::ConfigError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    /// Holds `config`, `keymap` and `themes`.
    pub config_root: PathBuf,
    /// Holds everything else.
    pub root: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub themes: PathBuf,
    pub avatars: PathBuf,
    pub keymap: PathBuf,
    pub icon_png: PathBuf,
}

impl AppPaths {
    /// One folder for everything (`--home`).
    pub fn from_root(root: PathBuf) -> Self {
        Self::split(root.clone(), root)
    }

    /// Settings under `config_root`, local files under `root`.
    pub fn split(config_root: PathBuf, root: PathBuf) -> Self {
        Self {
            config: config_root.join("config.toml"),
            keymap: config_root.join("keymap.toml"),
            themes: config_root.join("themes"),
            state: root.join("state.json"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            avatars: root.join("avatars"),
            icon_png: root.join("icon-256.png"),
            config_root,
            root,
        }
    }

    pub fn ensure_dirs(&self) -> Result<(), ConfigError> {
        for dir in [
            &self.config_root,
            &self.root,
            &self.cache,
            &self.logs,
            &self.themes,
            &self.avatars,
        ] {
            std::fs::create_dir_all(dir).map_err(|e| ConfigError::io(dir, &e))?;
        }
        Ok(())
    }

    /// `cache/<org>.catalog.json`.
    pub fn catalog_file(&self, org: &OrgId) -> PathBuf {
        self.cache.join(format!("{org}.catalog.json"))
    }
}

/// The two data folders. They are equal when one folder was chosen with `--home`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    pub config: PathBuf,
    pub local: PathBuf,
}

/// Pure: the caller passes the CLI value, the env value and the platform local data and config
/// dirs. An empty env value is ignored; relative paths are made absolute against `cwd`.
pub fn resolve_root(
    cli: Option<PathBuf>,
    env: Option<OsString>,
    data_local: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    cwd: &Path,
) -> Result<Roots, ConfigError> {
    let absolute = |path: PathBuf| {
        if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        }
    };
    let explicit = cli.or_else(|| env.filter(|v| !v.is_empty()).map(PathBuf::from));
    Ok(match explicit {
        Some(path) => {
            let path = absolute(path);
            Roots {
                config: path.clone(),
                local: path,
            }
        }
        None => Roots {
            config: absolute(
                config_dir
                    .ok_or(ConfigError::NoDataDir)?
                    .join(DATA_DIR_NAME),
            ),
            local: absolute(
                data_local
                    .ok_or(ConfigError::NoDataDir)?
                    .join(DATA_DIR_NAME),
            ),
        },
    })
}

#[cfg(test)]
mod tests;
