//! Finding `az`. A GUI app started before Azure CLI was installed may have a stale `PATH`, so
//! the standard install locations are checked too.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use prm_domain::auth::AuthError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    Unix,
}

impl HostOs {
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// Where `az` might be, best first. Pure: `on_path` is the `PATH` lookup result and `env` reads
/// environment variables, so every OS's rules are testable on any OS.
pub fn candidate_paths(
    os: HostOs,
    on_path: Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = on_path.into_iter().collect();
    match os {
        HostOs::Windows => {
            for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Some(root) = env(var) {
                    let wbin = Path::new(&root)
                        .join("Microsoft SDKs")
                        .join("Azure")
                        .join("CLI2")
                        .join("wbin");
                    paths.push(wbin.join("az.cmd"));
                }
            }
        }
        HostOs::Unix => {
            paths.extend(
                ["/opt/homebrew/bin/az", "/usr/local/bin/az", "/usr/bin/az"].map(PathBuf::from),
            );
        }
    }
    paths
}

/// Resolves `az` once and remembers it (until told to forget after a "not found").
#[derive(Debug, Default)]
pub struct AzLocator {
    fixed: Option<PathBuf>,
    cached: Mutex<Option<PathBuf>>,
}

impl AzLocator {
    /// Always uses `path` (tests, or a user override).
    pub fn fixed(path: PathBuf) -> Self {
        Self {
            fixed: Some(path),
            cached: Mutex::new(None),
        }
    }

    pub fn resolve(&self) -> Result<PathBuf, AuthError> {
        if let Some(path) = &self.fixed {
            return Ok(path.clone());
        }
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(path) = cached.as_ref() {
            return Ok(path.clone());
        }
        let env = |name: &str| std::env::var_os(name);
        let found = candidate_paths(HostOs::current(), which::which("az").ok(), &env)
            .into_iter()
            .find(|p| p.is_file())
            .ok_or(AuthError::AzNotInstalled)?;
        *cached = Some(found.clone());
        Ok(found)
    }

    /// Forget the cached path (it disappeared; maybe az was uninstalled or moved).
    pub fn forget(&self) {
        *self.cached.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}
