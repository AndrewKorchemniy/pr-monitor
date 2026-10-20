//! User themes on disk: loading `*.toml` from the themes folder, and a cheap change check run
//! when the window gains focus (no file watcher thread).

use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use crate::{
    file::parse_theme,
    issue::{IssueLevel, ThemeId, ThemeIssue},
    registry::ThemeEntry,
    tokens::ThemeSource,
};

/// The `*.toml` files in `dir`, sorted. A missing folder has none.
fn theme_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    files
}

/// Parses every user theme. Unreadable or invalid files become Error issues, never panics.
pub fn load_dir(dir: &Path) -> (Vec<(ThemeId, ThemeEntry)>, Vec<ThemeIssue>) {
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    for path in theme_files(dir) {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let id = ThemeId::new(stem);
        let parsed = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|src| {
                parse_theme(&src, stem)
                    .map(|(file, found)| (src, file, found))
                    .map_err(|e| e.to_string())
            });
        match parsed {
            Ok((src, file, found)) => {
                let entry = ThemeEntry {
                    file,
                    src: Arc::from(src),
                    source: ThemeSource::User(path.clone()),
                    issues: found,
                };
                entries.push((id, entry));
            }
            Err(message) => issues.push(ThemeIssue::new(&id, IssueLevel::Error, message)),
        }
    }
    (entries, issues)
}

/// Remembers a signature of the themes folder (names, sizes, modified times).
#[derive(Clone, Debug)]
pub struct ThemeDirScanner {
    dir: PathBuf,
    signature: u64,
}

impl ThemeDirScanner {
    pub fn new(dir: PathBuf) -> Self {
        let signature = signature(&dir);
        Self { dir, signature }
    }

    /// `true` when a theme file was added, changed or removed since the last call.
    pub fn changed(&mut self) -> bool {
        let now = signature(&self.dir);
        let changed = now != self.signature;
        self.signature = now;
        changed
    }
}

/// Never persisted, so the unspecified `DefaultHasher` algorithm is fine.
fn signature(dir: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    for path in theme_files(dir) {
        path.file_name().hash(&mut hasher);
        if let Ok(meta) = fs::metadata(&path) {
            meta.len().hash(&mut hasher);
            meta.modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests;
