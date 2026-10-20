//! Config errors (fatal to one operation) and config issues (reported, never fatal).

use std::{io, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("no local data directory")]
    NoDataDir,
    #[error("I/O error on {}: {message}", .path.display())]
    Io {
        path: PathBuf,
        message: Arc<str>,
        kind: io::ErrorKind,
    },
    #[error("cannot parse {}: {message}", .path.display())]
    Parse { path: PathBuf, message: Arc<str> },
    #[error("cannot serialize: {0}")]
    Serialize(Arc<str>),
    #[error("migration {from}→{to} failed: {message}")]
    Migration {
        from: u32,
        to: u32,
        message: Arc<str>,
    },
}

impl ConfigError {
    pub(crate) fn io(path: &std::path::Path, err: &io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            message: Arc::from(err.to_string()),
            kind: err.kind(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Something in the config was fixed up or ignored. Shown in Settings; never stops loading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigIssue {
    pub severity: Severity,
    /// Where in the file, e.g. `orgs[1].url`.
    pub path: Arc<str>,
    pub message: Arc<str>,
}

impl ConfigIssue {
    pub fn new(
        severity: Severity,
        path: impl Into<Arc<str>>,
        message: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            severity,
            path: path.into(),
            message: message.into(),
        }
    }
}
