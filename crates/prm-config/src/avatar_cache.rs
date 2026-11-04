//! The on-disk avatar cache: one small PNG per person under `<data>\avatars`.

use std::{
    io::{self, Read as _},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use prm_domain::{Timestamp, avatar::AvatarKey};

use crate::{error::ConfigError, store::atomic_write};

/// One cached picture and when it was fetched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedAvatar {
    /// The stored PNG. Empty means "this person has no picture", so it is not asked for again
    /// until the entry expires.
    pub png: Vec<u8>,
    pub stored_at: Timestamp,
}

/// Persistence port for avatars, so tests never touch the disk. Called on the blocking pool.
pub trait AvatarCache: Send + Sync + 'static {
    fn read(&self, key: &AvatarKey) -> Option<CachedAvatar>;
    fn write(&self, key: &AvatarKey, avatar: &CachedAvatar) -> Result<(), ConfigError>;
    /// Deletes every entry stored before `before`.
    fn prune(&self, before: Timestamp);
}

/// The on-disk [`AvatarCache`]. The fetch time is the file's modified time, set from the
/// caller's clock, so expiry never reads the wall clock.
#[derive(Clone, Debug)]
pub struct FileAvatarCache {
    dir: PathBuf,
}

impl FileAvatarCache {
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn file(&self, key: &AvatarKey) -> PathBuf {
        self.dir.join(format!("{}.png", key.stem()))
    }
}

impl AvatarCache for FileAvatarCache {
    fn read(&self, key: &AvatarKey) -> Option<CachedAvatar> {
        let mut file = std::fs::File::open(self.file(key)).ok()?;
        let stored_at = timestamp(file.metadata().ok()?.modified().ok()?)?;
        let mut png = Vec::new();
        file.read_to_end(&mut png).ok()?;
        Some(CachedAvatar { png, stored_at })
    }

    fn write(&self, key: &AvatarKey, avatar: &CachedAvatar) -> Result<(), ConfigError> {
        let path = self.file(key);
        atomic_write(&path, &avatar.png)?;
        set_modified(&path, avatar.stored_at).map_err(|e| ConfigError::io(&path, &e))
    }

    fn prune(&self, before: Timestamp) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_png = path.extension().is_some_and(|e| e == "png");
            if is_png && modified(&path).is_some_and(|at| at < before) {
                // Best effort: a file in use is pruned on a later start.
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

fn modified(path: &Path) -> Option<Timestamp> {
    timestamp(std::fs::metadata(path).ok()?.modified().ok()?)
}

fn timestamp(time: SystemTime) -> Option<Timestamp> {
    let secs = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Timestamp::from_unix(i64::try_from(secs).ok()?)
}

fn set_modified(path: &Path, at: Timestamp) -> io::Result<()> {
    let secs = u64::try_from(at.unix()).unwrap_or(0);
    let file = std::fs::File::options().write(true).open(path)?;
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}

#[cfg(test)]
mod tests;
