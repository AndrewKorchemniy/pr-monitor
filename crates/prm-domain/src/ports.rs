//! Ports the domain itself needs: time, opening URLs and self-updating. Implemented by
//! prm-service (`SystemClock`), prm-platform (URL opener) and the binary (updater); faked in
//! `testing`.

use std::{ffi::OsString, sync::Arc};

use crate::{
    time::Timestamp,
    update::{ApplyMode, DisabledReason, UpdateOffer},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PortErrorKind {
    Unsupported,
    Denied,
    NotFound,
    Failed,
}

/// A failure from an OS adapter. `message` is technical detail for logs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct PortError {
    pub kind: PortErrorKind,
    pub message: Arc<str>,
}

impl PortError {
    pub fn new(kind: PortErrorKind, message: impl Into<Arc<str>>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Wall-clock time. Domain logic takes `Timestamp`s; only services hold a `Clock`.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Timestamp;

    /// The local UTC offset, for display only.
    fn local_offset(&self) -> ::time::UtcOffset {
        ::time::UtcOffset::UTC
    }
}

/// Opens an allow-listed https URL in the user's browser.
pub trait UrlOpener: Send + Sync + 'static {
    fn open(&self, url: &url::Url) -> Result<(), PortError>;
}

/// Looks for, downloads and applies a newer release of this app. Every call blocks, so the
/// service runs them on the blocking pool.
pub trait Updater: Send + Sync + 'static {
    /// Whether updating can work in this run, decided once and cheap to ask.
    fn availability(&self) -> Result<(), DisabledReason>;

    /// The version of a downloaded package that waits for a restart.
    fn pending(&self) -> Option<Arc<str>>;

    /// `Some` when a newer release exists; no newer release is not an error.
    fn check(&self) -> Result<Option<UpdateOffer>, PortError>;

    /// Downloads the offer that the last `check` returned, calling `progress` with 0 to 100.
    fn download(
        &self,
        offer: &UpdateOffer,
        progress: &(dyn Fn(u8) + Sync),
    ) -> Result<(), PortError>;

    /// Starts the updater so that it applies the pending package once this process has exited,
    /// then starts the app again with `restart_args`. It does not exit the process.
    fn apply_on_exit(&self, restart_args: &[OsString], mode: ApplyMode) -> Result<(), PortError>;
}
