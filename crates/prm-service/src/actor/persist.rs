//! Writing `state.json`: debounced, at most one save in flight, and a "flush" that saves before
//! showing a cycle's toasts.

use prm_config::{ConfigError, PersistedState, StateRepo, state::STATE_SCHEMA};
use prm_domain::{model::Catalog, notify::NotifierOp};
use tokio::{
    task::{AbortHandle, Id},
    time::Instant,
};

use super::Service;
use crate::{
    consts::{PERSIST_DEBOUNCE, SHUTDOWN_SAVE_WAIT},
    errors::UiError,
    jobs::{JobKind, JobOutput, JobTag, Saved, apply_notifier_ops, blocking},
    publish,
};

#[derive(Default)]
pub(super) struct PersistState {
    pub deadline: Option<Instant>,
    /// The running state save (plain or flush); saves never overlap, so an older state can
    /// never land after a newer one.
    in_flight: Option<Id>,
    /// Dirtied again while a save was running: save once more afterwards.
    again: bool,
    /// Toasts of cycles that finished while a save was running: flushed after it.
    queued: Option<Vec<NotifierOp>>,
    /// The last state save failed; a banner says so until a save succeeds.
    pub failed: bool,
}

impl Service {
    /// Schedules a save within `PERSIST_DEBOUNCE` (bounded latency, coalesced writes).
    pub(super) fn mark_persist(&mut self) {
        if self.persist.deadline.is_none() {
            self.persist.deadline = Some(Instant::now() + PERSIST_DEBOUNCE);
        }
    }

    pub(super) fn start_persist(&mut self) {
        self.persist.deadline = None;
        if self.persist.in_flight.is_some() {
            self.persist.again = true;
            return;
        }
        let state = self.build_persisted();
        let job = self.spawn_save(JobTag::global(JobKind::Save), Saved::State, move |repo| {
            repo.save_state(&state)
        });
        self.persist.in_flight = Some(job.id());
    }

    /// Saves now, then shows the toasts, so a restart does not toast the same changes again. A
    /// crash in between loses a toast; a failed save can still duplicate one after a restart.
    pub(super) fn flush(&mut self, ops: Vec<NotifierOp>) {
        if self.persist.in_flight.is_some() {
            self.persist.queued.get_or_insert_default().extend(ops);
            return;
        }
        self.persist.deadline = None;
        self.persist.again = false;
        let state = self.build_persisted();
        let repo = self.deps.repo.clone();
        let notifier = self.deps.notifier.clone();
        let job = self.spawn(JobTag::global(JobKind::Flush), async move {
            let saved = save_result(blocking(move || repo.save_state(&state)).await);
            let failures = blocking(move || apply_notifier_ops(notifier.as_ref(), &ops)).await;
            JobOutput::Flushed {
                saved,
                notify_failures: failures.unwrap_or(1),
            }
        });
        self.persist.in_flight = Some(job.id());
    }

    /// A state save (plain or flush) finished: start whatever waited for it.
    pub(super) fn on_state_saved(&mut self, result: Result<(), UiError>) {
        self.persist.in_flight = None;
        self.persist.failed = result.is_err();
        if let Err(error) = result {
            tracing::warn!(error = %error.title, "state save failed; retrying");
            self.mark_persist();
        }
        if let Some(ops) = self.persist.queued.take() {
            self.flush(ops);
        } else if std::mem::take(&mut self.persist.again) {
            self.persist.deadline = Some(Instant::now());
        }
    }

    /// A save job panicked; only the running state save holds anything up.
    pub(super) fn on_save_panicked(&mut self, job: Id) {
        if self.persist.in_flight == Some(job) {
            self.on_state_saved(Err(UiError::internal("save task panicked")));
        }
    }

    /// At shutdown: lets a running save finish (bounded), so the final save is the last write.
    pub(super) async fn wait_for_save(&mut self) {
        let Some(id) = self.persist.in_flight else {
            return;
        };
        if tokio::time::timeout(SHUTDOWN_SAVE_WAIT, self.join_until(id))
            .await
            .is_err()
        {
            tracing::warn!("state save still running at shutdown");
        }
    }

    /// Drains finished jobs (their results no longer matter) until `job` is one of them.
    async fn join_until(&mut self, job: Id) {
        while let Some(joined) = self.jobs.join_next_with_id().await {
            if joined.map_or_else(|error| error.id(), |(id, _)| id) == job {
                return;
            }
        }
    }

    pub(super) fn build_persisted(&self) -> PersistedState {
        let orgs = self
            .orgs
            .iter()
            .map(|o| {
                let method = o.link.auth.method();
                (o.data.id.clone(), o.data.to_persisted(method))
            })
            .collect();
        PersistedState {
            schema_version: STATE_SCHEMA,
            saved_at: Some(self.deps.clock.now()),
            notifications_paused_until: self.paused_until,
            orgs,
            seen: publish::prune_seen(&self.seen, self.orgs.iter().map(|o| &o.data)),
            ui: (*self.ui_state).clone(),
        }
    }

    pub(super) fn save_catalog(&mut self, idx: usize, catalog: &Catalog) {
        let org = self.orgs[idx].data.id.clone();
        let catalog = catalog.clone();
        let tag = JobTag::org(&org, JobKind::Save);
        self.spawn_save(tag, Saved::Catalog, move |repo| {
            repo.save_catalog(&org, &catalog)
        });
    }

    /// Runs one repository write on the blocking pool and reports it as `what`.
    pub(super) fn spawn_save<F>(&mut self, tag: JobTag, what: Saved, save: F) -> AbortHandle
    where
        F: FnOnce(&dyn StateRepo) -> Result<(), ConfigError> + Send + 'static,
    {
        let repo = self.deps.repo.clone();
        self.spawn(tag, async move {
            let result = blocking(move || save(repo.as_ref())).await;
            JobOutput::Saved {
                what,
                result: save_result(result),
            }
        })
    }
}

fn save_result(result: Option<Result<(), ConfigError>>) -> Result<(), UiError> {
    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => Err(UiError::from(&error)),
        None => Err(UiError::internal("save task failed")),
    }
}
