//! Self-updating: the schedule of checks, the download with its progress, and the restart that
//! applies it. Each step is one job on the blocking pool; the actor only moves the status.

use std::{sync::Arc, time::Duration};

use prm_domain::{
    ports::PortError,
    update::{ApplyMode, UpdateOffer, UpdateStage, UpdateStatus},
};
use tokio::time::Instant;

use super::Service;
use crate::{
    event::AppEvent,
    jobs::{JobKind, JobOutput, JobTag, port_call},
    updates::{
        APPLY_TIMEOUT, CHECK_INTERVAL, CHECK_TIMEOUT, DOWNLOAD_TIMEOUT, FIRST_CHECK,
        RETRY_AFTER_FAILURE,
    },
};

impl Service {
    /// Reads whether updating works here, looks for a package that is already downloaded, and
    /// schedules the first automatic check.
    pub(super) fn start_updates(&mut self) {
        if let Err(reason) = self.deps.updater.availability() {
            tracing::info!(?reason, "updating is not available");
            self.updates.status = UpdateStatus::Disabled(reason);
            return;
        }
        let updater = self.deps.updater.clone();
        self.spawn(JobTag::global(JobKind::Update), async move {
            let pending = port_call(CHECK_TIMEOUT, move || Ok(updater.pending())).await;
            JobOutput::UpdatePending(pending)
        });
        self.schedule_update_check(FIRST_CHECK);
    }

    /// An automatic check is wanted after `delay`, when they are on and there is something to do.
    fn schedule_update_check(&mut self, delay: Duration) {
        let wanted = self.config.updates.auto && self.updates.is_active();
        self.updates.next_check = wanted.then(|| Instant::now() + delay);
    }

    /// The scheduled check is due.
    pub(super) fn start_due_update_check(&mut self, now: Instant) {
        if self.updates.next_check.is_none_or(|at| at > now) {
            return;
        }
        self.updates.next_check = None;
        if self.updates.occupied.is_set() {
            // why: an earlier call that timed out is still running; look again later.
            self.schedule_update_check(RETRY_AFTER_FAILURE);
        } else {
            self.check_for_updates();
        }
    }

    /// `CheckForUpdates` and the schedule both end up here; busy, ready and disabled do nothing.
    pub(super) fn check_for_updates(&mut self) {
        let busy = self.updates.in_flight || self.updates.occupied.is_set();
        if busy || !self.updates.status.can_check() {
            return;
        }
        self.updates.status = UpdateStatus::Checking;
        self.updates.in_flight = true;
        // why: a scheduled check starts on a timer, not a command, so nothing else publishes it.
        self.publisher.dirty = true;
        let updater = self.deps.updater.clone();
        let running = self.updates.occupied.enter();
        self.spawn(JobTag::global(JobKind::Update), async move {
            let result = port_call(CHECK_TIMEOUT, move || {
                let _running = running;
                updater.check()
            })
            .await;
            JobOutput::UpdateChecked(result)
        });
    }

    fn download_update(&mut self, offer: UpdateOffer) {
        self.updates.status = UpdateStatus::downloading(0);
        let updater = self.deps.updater.clone();
        let progress = self.updates.progress_tx.clone();
        let running = self.updates.occupied.enter();
        self.spawn(JobTag::global(JobKind::Update), async move {
            let version = offer.version.clone();
            // why: the receiver only goes away when the service stops.
            let report = move |percent: u8| {
                let _ = progress.send(percent);
            };
            let result = port_call(DOWNLOAD_TIMEOUT, move || {
                let _running = running;
                updater.download(&offer, &report)
            })
            .await;
            JobOutput::UpdateDownloaded { version, result }
        });
    }

    pub(super) fn on_update_checked(&mut self, result: Result<Option<UpdateOffer>, PortError>) {
        match result {
            Ok(Some(offer)) => {
                tracing::info!(version = %offer.version, "update found");
                self.download_update(offer);
            }
            Ok(None) => {
                self.updates.in_flight = false;
                self.updates.status = UpdateStatus::UpToDate;
                self.schedule_update_check(CHECK_INTERVAL);
            }
            Err(error) => {
                self.updates.in_flight = false;
                self.update_failed(UpdateStage::Check, &error);
            }
        }
    }

    pub(super) fn on_update_downloaded(
        &mut self,
        version: Arc<str>,
        result: Result<(), PortError>,
    ) {
        self.updates.in_flight = false;
        match result {
            Ok(()) => {
                tracing::info!(%version, "update downloaded");
                self.set_update_ready(version);
            }
            Err(error) => self.update_failed(UpdateStage::Download, &error),
        }
    }

    /// Progress of the running download; late values after it ended are dropped.
    pub(super) fn on_update_progress(&mut self, percent: u8) {
        let next = UpdateStatus::downloading(percent);
        let moved = matches!(self.updates.status, UpdateStatus::Downloading(_))
            && self.updates.status != next;
        if moved {
            self.updates.status = next;
            self.publisher.progress_dirty = true;
        }
    }

    pub(super) fn on_update_pending(&mut self, pending: Result<Option<Arc<str>>, PortError>) {
        let version = match pending {
            Ok(Some(version)) => version,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(error = %error.message, "could not look for a downloaded update");
                return;
            }
        };
        if self.updates.status.can_check() {
            tracing::info!(%version, "an update is waiting for a restart");
            self.set_update_ready(version);
        }
    }

    /// `ApplyUpdate`: the updater waits for this process to exit, so the UI quits next.
    pub(super) fn apply_update(&mut self) {
        if self.updates.in_flight || self.updates.status.ready_version().is_none() {
            return;
        }
        self.updates.in_flight = true;
        let updater = self.deps.updater.clone();
        self.spawn(JobTag::global(JobKind::Update), async move {
            let result = port_call(APPLY_TIMEOUT, move || {
                updater.apply_on_exit(&[], ApplyMode::Interactive)
            })
            .await;
            JobOutput::UpdateApplied(result)
        });
    }

    pub(super) fn on_update_applied(&mut self, result: Result<(), PortError>) {
        self.updates.in_flight = false;
        match result {
            Ok(()) => self.deps.events.emit(AppEvent::QuitForUpdate),
            Err(error) => self.update_failed(UpdateStage::Apply, &error),
        }
    }

    /// The `[updates] auto` setting changed: on, a check comes soon; off, none is scheduled.
    pub(super) fn updates_config_changed(&mut self) {
        self.schedule_update_check(FIRST_CHECK);
    }

    /// An update job panicked: a check or download in progress shows as failed; any other
    /// status (a ready update, above all) stays as it was.
    pub(super) fn on_update_panicked(&mut self) {
        self.updates.in_flight = false;
        let stage = match self.updates.status {
            UpdateStatus::Checking => UpdateStage::Check,
            UpdateStatus::Downloading(_) => UpdateStage::Download,
            UpdateStatus::Idle
            | UpdateStatus::UpToDate
            | UpdateStatus::Ready(_)
            | UpdateStatus::Failed(_)
            | UpdateStatus::Disabled(_) => return,
        };
        self.fail_update(stage);
    }

    fn update_failed(&mut self, stage: UpdateStage, error: &PortError) {
        tracing::warn!(?stage, error = %error.message, "update step failed");
        self.fail_update(stage);
    }

    fn fail_update(&mut self, stage: UpdateStage) {
        self.updates.status = UpdateStatus::Failed(stage);
        self.schedule_update_check(RETRY_AFTER_FAILURE);
    }

    /// A downloaded update waits for a restart; there is nothing more to check for.
    fn set_update_ready(&mut self, version: Arc<str>) {
        self.updates.status = UpdateStatus::Ready(version);
        self.updates.next_check = None;
    }
}
