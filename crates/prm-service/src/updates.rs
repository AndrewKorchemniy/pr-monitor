//! When the service looks for updates, how long it waits for the updater, and the state it
//! keeps between those calls. The decisions here are pure; `actor/updates.rs` carries them out.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use prm_domain::update::UpdateStatus;
use tokio::{sync::mpsc, time::Instant};

/// The first automatic check comes this long after the service starts, so a login is not slowed.
pub(crate) const FIRST_CHECK: Duration = Duration::from_secs(60);
/// Between automatic checks that worked.
pub(crate) const CHECK_INTERVAL: Duration = Duration::from_hours(6);
/// After a failed check or download. GitHub allows 60 API calls an hour per address, and a
/// check costs one, so an hour is safe even for an office behind one address.
pub(crate) const RETRY_AFTER_FAILURE: Duration = Duration::from_hours(1);
/// The updater's HTTP client has no timeout of its own, so a check that takes longer than this
/// counts as failed.
pub(crate) const CHECK_TIMEOUT: Duration = Duration::from_mins(2);
/// The same for a download (a full package is about 20 MB, so this is generous).
pub(crate) const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(30);
/// Starting the updater only launches a process.
pub(crate) const APPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether an updater call still occupies a blocking thread. A job that timed out stops
/// waiting, but its thread keeps running; until it returns, no new call starts, so a hung
/// connection costs one thread of the small blocking pool, not one per retry.
#[derive(Clone, Debug, Default)]
pub(crate) struct Occupied(Arc<AtomicBool>);

impl Occupied {
    /// Marks a call as running until the returned guard is dropped (move it into the call).
    pub(crate) fn enter(&self) -> OccupiedGuard {
        self.0.store(true, Ordering::SeqCst);
        OccupiedGuard(Arc::clone(&self.0))
    }

    pub(crate) fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Clears [`Occupied`] when the call ends or never ran.
#[derive(Debug)]
pub(crate) struct OccupiedGuard(Arc<AtomicBool>);

impl Drop for OccupiedGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// What the actor keeps for updating.
pub(crate) struct UpdateRuntime {
    pub status: UpdateStatus,
    /// The next automatic check; `None` when automatic checks are off, or nothing is left to do
    /// (updating is disabled, or an update waits for a restart).
    pub next_check: Option<Instant>,
    /// A check, download or apply job is running.
    pub in_flight: bool,
    pub occupied: Occupied,
    /// Percent values from a running download.
    pub progress_tx: mpsc::UnboundedSender<u8>,
    pub progress_rx: mpsc::UnboundedReceiver<u8>,
}

impl UpdateRuntime {
    pub(crate) fn new() -> Self {
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        Self {
            status: UpdateStatus::default(),
            next_check: None,
            in_flight: false,
            occupied: Occupied::default(),
            progress_tx,
            progress_rx,
        }
    }

    /// Updating can still do something: it is possible here and no update waits for a restart.
    pub(crate) const fn is_active(&self) -> bool {
        !matches!(
            self.status,
            UpdateStatus::Disabled(_) | UpdateStatus::Ready(_)
        )
    }
}

#[cfg(test)]
mod tests;
