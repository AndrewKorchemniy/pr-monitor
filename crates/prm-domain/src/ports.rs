//! Ports the domain itself needs: time and opening URLs. Implemented by prm-service
//! (`SystemClock`) and prm-platform (URL opener); faked in `testing`.

use std::sync::Arc;

use crate::time::Timestamp;

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
