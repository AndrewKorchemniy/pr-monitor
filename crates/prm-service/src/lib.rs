//! The service layer: a single actor on its own thread that schedules refreshes, classifies and
//! diffs PRs, plans toasts, persists state and publishes an immutable `AppState` for the UI.
//! Also re-exports every port, so adapters and the bin depend on one crate.
#![forbid(unsafe_code)]

mod actor;
mod auth_machine;
mod clock;
mod command;
pub mod consts;
mod errors;
mod event;
mod handle;
mod jobs;
mod org;
pub mod ports;
mod publish;
mod scheduler;
pub mod state;
mod text;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use clock::SystemClock;
pub use command::{Command, CommandSender};
pub use errors::UiError;
pub use event::{AppEvent, ChannelEventSink, EventSink, Notice, SearchPhase, channel_sink};
pub use handle::{InitialData, ServiceDeps, ServiceHandle, build, initial_state, spawn};
pub use state::{AppState, AuthStateView, OrgHealth, OrgStatus, RefreshPhase};

#[cfg(test)]
mod tests;
