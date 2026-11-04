//! Hand-written fakes for this crate's ports, for tests here and in dependent crates.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use prm_domain::{OrgId, Timestamp, avatar::AvatarKey, model::Catalog};

use crate::{
    avatar_cache::{AvatarCache, CachedAvatar},
    error::ConfigError,
    schema::Config,
    state::PersistedState,
    store::StateRepo,
};

/// An in-memory [`StateRepo`] for tests: records saves and can fail the next one.
#[derive(Debug, Default)]
pub struct MemoryRepo {
    saved: Mutex<Vec<Saved>>,
    fail_next: Mutex<Option<ConfigError>>,
}

/// One write recorded by [`MemoryRepo`].
#[derive(Clone, Debug, PartialEq)]
pub enum Saved {
    Config(Box<Config>),
    State(Box<PersistedState>),
    Catalog(OrgId),
    Deleted(OrgId),
}

impl MemoryRepo {
    /// Every successful write, oldest first.
    pub fn saved(&self) -> Vec<Saved> {
        self.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The most recently saved state, if any.
    pub fn last_state(&self) -> Option<PersistedState> {
        self.saved().into_iter().rev().find_map(|s| match s {
            Saved::State(state) => Some(*state),
            Saved::Config(_) | Saved::Catalog(_) | Saved::Deleted(_) => None,
        })
    }

    /// Makes the next write of any kind fail with `error`.
    pub fn fail_next(&self, error: ConfigError) {
        *self
            .fail_next
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error);
    }

    fn record(&self, saved: Saved) -> Result<(), ConfigError> {
        let failure = self
            .fail_next
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(error) = failure {
            return Err(error);
        }
        self.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(saved);
        Ok(())
    }
}

impl StateRepo for MemoryRepo {
    fn save_config(&self, cfg: &Config) -> Result<(), ConfigError> {
        self.record(Saved::Config(Box::new(cfg.clone())))
    }

    fn save_state(&self, state: &PersistedState) -> Result<(), ConfigError> {
        self.record(Saved::State(Box::new(state.clone())))
    }

    fn save_catalog(&self, org: &OrgId, _catalog: &Catalog) -> Result<(), ConfigError> {
        self.record(Saved::Catalog(org.clone()))
    }

    fn delete_org_data(&self, org: &OrgId) -> Result<(), ConfigError> {
        self.record(Saved::Deleted(org.clone()))
    }
}

/// An in-memory [`AvatarCache`] for tests.
#[derive(Debug, Default)]
pub struct MemoryAvatarCache {
    entries: Mutex<HashMap<AvatarKey, CachedAvatar>>,
}

impl MemoryAvatarCache {
    /// Stores an entry as if it had been written earlier.
    pub fn seed(&self, key: AvatarKey, avatar: CachedAvatar) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, avatar);
    }

    pub fn get(&self, key: &AvatarKey) -> Option<CachedAvatar> {
        self.read(key)
    }

    pub fn is_empty(&self) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }
}

impl AvatarCache for MemoryAvatarCache {
    fn read(&self, key: &AvatarKey) -> Option<CachedAvatar> {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    fn write(&self, key: &AvatarKey, avatar: &CachedAvatar) -> Result<(), ConfigError> {
        self.seed(key.clone(), avatar.clone());
        Ok(())
    }

    fn prune(&self, before: Timestamp) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, a| a.stored_at >= before);
    }
}
