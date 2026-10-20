//! All available themes (built-in plus user files) and resolving the user's selection to the
//! dark and light tokens in use.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use crate::{
    builtin::builtin_sources,
    file::{ThemeFile, parse_theme},
    issue::{IssueLevel, ThemeId, ThemeIssue},
    metrics::Density,
    scan::load_dir,
    tokens::{ThemeSource, Tokens},
};

pub const DEFAULT_DARK: &str = "graphite";
pub const DEFAULT_LIGHT: &str = "vellum";
/// `ThemeSelection::theme` value that follows the OS appearance.
pub const SYSTEM: &str = "system";

/// One parsed theme file.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeEntry {
    pub file: ThemeFile,
    pub src: Arc<str>,
    pub source: ThemeSource,
    /// Problems found while parsing (validation runs when tokens are built).
    pub issues: Vec<ThemeIssue>,
}

#[derive(Clone, Debug, Default)]
pub struct ThemeRegistry {
    entries: BTreeMap<ThemeId, ThemeEntry>,
}

impl ThemeRegistry {
    /// The compiled-in themes. A built-in that fails to parse is logged and skipped (tests
    /// forbid it).
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for (stem, src) in builtin_sources() {
            match parse_theme(src, stem) {
                Ok((file, issues)) => {
                    let entry = ThemeEntry {
                        file,
                        src: Arc::from(*src),
                        source: ThemeSource::Builtin,
                        issues,
                    };
                    registry.entries.insert(ThemeId::new(stem), entry);
                }
                Err(error) => tracing::error!(%error, "built-in theme failed to parse"),
            }
        }
        registry
    }

    /// Adds the user's themes. A user file with a built-in's id replaces the built-in (Info).
    pub fn with_user_dir(mut self, dir: &Path) -> (Self, Vec<ThemeIssue>) {
        let (entries, mut issues) = load_dir(dir);
        for (id, entry) in entries {
            if let Some(replaced) = self.entries.get(&id) {
                issues.push(ThemeIssue::new(
                    &id,
                    IssueLevel::Info,
                    format!("replaces the built-in {}", replaced.file.name),
                ));
            }
            self.entries.insert(id, entry);
        }
        (self, issues)
    }

    pub fn list(&self) -> impl Iterator<Item = (&ThemeId, &ThemeEntry)> {
        self.entries.iter()
    }

    pub fn get(&self, id: &str) -> Option<&ThemeEntry> {
        self.entries.get(&ThemeId::new(id))
    }

    /// Built tokens plus every issue (parse and validation).
    pub fn tokens(&self, id: &str, density: Density) -> Option<(Tokens, Vec<ThemeIssue>)> {
        let entry = self.get(id)?;
        let (tokens, mut issues) =
            Tokens::build(&entry.file, &entry.src, entry.source.clone(), density);
        issues.splice(0..0, entry.issues.iter().cloned());
        Some((tokens, issues))
    }
}

/// What the user picked in Settings › Appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSelection {
    /// `"system"` or a theme id.
    pub theme: String,
    /// Used for a dark OS appearance when following the system.
    pub dark: String,
    /// Used for a light OS appearance when following the system.
    pub light: String,
    pub density: Density,
}

/// The tokens to use now, for either OS appearance.
#[derive(Clone, Debug)]
pub struct ThemeSet {
    pub follow_system: bool,
    pub dark: Arc<Tokens>,
    pub light: Arc<Tokens>,
    pub issues: Vec<ThemeIssue>,
}

/// `"system"` uses the dark and light picks; an explicit id fills both slots. Unknown ids fall
/// back to the defaults with a Warning.
pub fn resolve(registry: &ThemeRegistry, selection: &ThemeSelection) -> ThemeSet {
    let mut issues = Vec::new();
    let mut pick = |id: &str, fallback: &str| -> Arc<Tokens> {
        Arc::new(load_or_fallback(
            registry,
            id,
            fallback,
            selection.density,
            &mut issues,
        ))
    };
    if selection.theme == SYSTEM {
        let dark = pick(&selection.dark, DEFAULT_DARK);
        let light = pick(&selection.light, DEFAULT_LIGHT);
        return ThemeSet {
            follow_system: true,
            dark,
            light,
            issues,
        };
    }
    let chosen = pick(&selection.theme, DEFAULT_DARK);
    ThemeSet {
        follow_system: false,
        dark: chosen.clone(),
        light: chosen,
        issues,
    }
}

fn load_or_fallback(
    registry: &ThemeRegistry,
    id: &str,
    fallback: &str,
    density: Density,
    issues: &mut Vec<ThemeIssue>,
) -> Tokens {
    if let Some((tokens, found)) = registry.tokens(id, density) {
        issues.extend(found);
        return tokens;
    }
    issues.push(ThemeIssue::new(
        &ThemeId::new(id),
        IssueLevel::Warning,
        format!("theme {id:?} not found; using {fallback:?}"),
    ));
    let (tokens, found) = registry
        .tokens(fallback, density)
        .unwrap_or_else(|| last_resort(density));
    issues.extend(found);
    tokens
}

/// Reached only if the default theme itself is missing from the registry.
#[expect(
    clippy::expect_used,
    reason = "built-ins are compiled in; builtin_themes_parse_without_issues covers both expects"
)]
fn last_resort(density: Density) -> (Tokens, Vec<ThemeIssue>) {
    let (stem, src) = builtin_sources()
        .first()
        .copied()
        .expect("at least one built-in theme");
    let (file, issues) = parse_theme(src, stem).expect("built-in themes parse");
    let (tokens, mut built) = Tokens::build(&file, src, ThemeSource::Builtin, density);
    built.extend(issues);
    (tokens, built)
}

#[cfg(test)]
mod tests;
