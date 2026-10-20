//! Pure, synchronous domain model and rules for PR Monitor: ids, classification, diffing,
//! notification policy and the dashboard view model. No IO, no async, no UI.

pub mod app;
pub mod auth;
pub mod classify;
pub mod diff;
pub mod hash;
pub mod ids;
pub mod model;
pub mod person;
pub mod plan;
pub mod ports;
pub mod scope;
pub mod secret;
pub mod snapshot;
pub mod time;
pub mod tombstone;
pub mod uri;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use classify::{ClassifyConfig, PrStatus};
pub use ids::{OrgId, PrKey, Vsid};
pub use person::Person;
pub use secret::Secret;
pub use time::Timestamp;
