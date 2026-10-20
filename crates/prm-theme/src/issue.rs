//! Theme identity and the problems found while loading a theme (never fatal).

use std::{fmt, sync::Arc};

/// A theme's id: the file stem of its TOML file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeId(pub Arc<str>);

impl ThemeId {
    pub fn new(id: &str) -> Self {
        Self(Arc::from(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueLevel {
    Info,
    Warning,
    Error,
}

/// Shown in Settings › Appearance next to the theme.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeIssue {
    pub theme: ThemeId,
    pub level: IssueLevel,
    /// The palette key involved, when there is one.
    pub key: Option<&'static str>,
    pub message: String,
}

impl ThemeIssue {
    pub fn new(theme: &ThemeId, level: IssueLevel, message: impl Into<String>) -> Self {
        Self {
            theme: theme.clone(),
            level,
            key: None,
            message: message.into(),
        }
    }
}
