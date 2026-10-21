//! `Tokens`: a ready-to-use theme (palette + metrics), built from a theme file.

use std::{path::PathBuf, sync::Arc};

use crate::{
    derive::derive_palette,
    file::{Appearance, ThemeFile},
    issue::{IssueLevel, ThemeId, ThemeIssue},
    metrics::Metrics,
    palette::Palette,
    validate::validate,
};

/// Where a theme came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    Builtin,
    User(PathBuf),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub id: ThemeId,
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub appearance: Appearance,
    pub palette: Palette,
    pub metrics: Metrics,
    pub source: ThemeSource,
    /// The original file text, for "Copy theme as TOML".
    pub file_toml: Arc<str>,
}

impl Tokens {
    /// Derive, apply overrides (an unknown key is a Warning), then validate. Never panics.
    pub fn build(file: &ThemeFile, src: &str, source: ThemeSource) -> (Self, Vec<ThemeIssue>) {
        let id = ThemeId::new(&file.id);
        let mut palette = derive_palette(file);
        let mut issues = Vec::new();
        for (key, color) in &file.overrides {
            if !palette.set(key, *color) {
                issues.push(ThemeIssue::new(
                    &id,
                    IssueLevel::Warning,
                    format!("unknown palette key {key:?} in [overrides]"),
                ));
            }
        }
        issues.extend(validate(&id, &palette));
        let tokens = Self {
            name: Arc::from(file.name.as_str()),
            description: Arc::from(file.description.as_str()),
            appearance: file.appearance,
            metrics: Metrics::new(&file.metrics),
            palette,
            source,
            file_toml: Arc::from(src),
            id,
        };
        (tokens, issues)
    }
}

#[cfg(test)]
mod tests;
