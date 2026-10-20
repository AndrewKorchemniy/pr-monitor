//! PR Monitor's files: `config.toml` (user settings, multi-org), `state.json` (machine state),
//! catalog caches, and where they live. Loads never fail; writes are atomic.
#![forbid(unsafe_code)]

pub mod diff;
pub mod error;
pub mod limits;
pub mod migrate;
pub mod paths;
pub mod schema;
pub mod state;
pub mod store;
pub mod validate;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use diff::{ConfigChanges, OrgChanges, diff_configs};
pub use error::{ConfigError, ConfigIssue, Severity};
pub use paths::{AppPaths, resolve_root};
pub use schema::{Config, OrgProfile, UiPrefs};
pub use state::{OrgPersisted, PersistedState, UiState, WindowRect};
pub use store::{
    ConfigLoad, FileRepo, StateLoad, StateRepo, atomic_write, load_catalogs, load_config,
    load_state, save_config,
};
pub use validate::normalize;
