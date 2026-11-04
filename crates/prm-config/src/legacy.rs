//! The one-time move of the old single data folder (`%LOCALAPPDATA%\PRMonitor`) into the split
//! layout: settings into the config folder, everything else into the local folder. Nothing here
//! is fatal and nothing is overwritten: an entry that cannot move stays where it is and the next
//! start tries again.

use std::{ffi::OsStr, fs, io, path::Path};

/// Entries that belong in the config folder; everything else goes to the local folder.
const CONFIG_ENTRIES: [&str; 3] = ["config.toml", "keymap.toml", "themes"];
/// Left over from the instance that ran there; the new folder has its own, so it is deleted.
const DISCARDED: [&str; 1] = ["instance.lock"];

/// What one run did, for the log.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LegacyMove {
    /// The legacy folder existed.
    pub found: bool,
    pub moved: Vec<String>,
    /// Left in place because the destination already exists.
    pub skipped: Vec<String>,
    /// `name: error` for each entry that could not move; retried on the next start.
    pub failed: Vec<String>,
    /// The legacy folder was empty afterwards and has been removed.
    pub removed: bool,
}

/// Moves what is in `legacy` into `config_root` and `root`. A missing folder is a no-op.
pub fn move_legacy_folder(legacy: &Path, config_root: &Path, root: &Path) -> LegacyMove {
    let mut outcome = LegacyMove::default();
    let entries = match fs::read_dir(legacy) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return outcome,
        Err(error) => {
            outcome.found = true;
            outcome
                .failed
                .push(format!("{}: {error}", legacy.display()));
            return outcome;
        }
    };
    outcome.found = true;
    for entry in entries {
        match entry {
            Ok(entry) => move_entry(&entry.path(), config_root, root, &mut outcome),
            Err(error) => outcome
                .failed
                .push(format!("{}: {error}", legacy.display())),
        }
    }
    // why: `remove_dir` only succeeds on an empty folder, which is exactly when it should go.
    outcome.removed = fs::remove_dir(legacy).is_ok();
    outcome
}

fn listed(name: &OsStr, names: &[&str]) -> bool {
    name.to_str()
        .is_some_and(|name| names.iter().any(|n| n.eq_ignore_ascii_case(name)))
}

fn move_entry(src: &Path, config_root: &Path, root: &Path, outcome: &mut LegacyMove) {
    let Some(name) = src.file_name() else {
        return;
    };
    let label = name.to_string_lossy().into_owned();
    if listed(name, &DISCARDED) {
        if let Err(error) = remove_any(src) {
            outcome.failed.push(format!("{label}: {error}"));
        }
        return;
    }
    let parent = if listed(name, &CONFIG_ENTRIES) {
        config_root
    } else {
        root
    };
    let dest = parent.join(name);
    // why: `ensure_dirs` makes the empty `cache`, `logs`, `themes` and `avatars` folders on every
    // start, after this move. One left by a failed earlier attempt must not block the retry.
    // `remove_dir` only succeeds on an empty folder, so no data is removed.
    let _ = fs::remove_dir(&dest);
    if dest.symlink_metadata().is_ok() {
        outcome.skipped.push(label);
        return;
    }
    match relocate(src, &dest) {
        Ok(()) => outcome.moved.push(label),
        Err(error) => outcome.failed.push(format!("{label}: {error}")),
    }
}

/// Rename, or copy then delete when the rename fails (another volume, a held handle).
fn relocate(src: &Path, dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    relocate_by_copy(src, dest)
}

/// Copies `src` to `dest`, then removes `src`. A failed copy leaves `src` as it was and no `dest`,
/// so a retry starts clean. A failed removal of `src` keeps the complete copy in `dest`, so
/// nothing is lost.
fn relocate_by_copy(src: &Path, dest: &Path) -> io::Result<()> {
    if let Err(error) = copy_tree(src, dest) {
        let _ = remove_any(dest);
        return Err(error);
    }
    remove_any(src)
}

fn copy_tree(src: &Path, dest: &Path) -> io::Result<()> {
    if !src.is_dir() {
        return fs::copy(src, dest).map(|_| ());
    }
    fs::create_dir(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
    }
    Ok(())
}

fn remove_any(path: &Path) -> io::Result<()> {
    let removed = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match removed {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests;
