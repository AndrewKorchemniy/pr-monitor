//! Writing `state.json`: debounced, at most one save in flight, and a "flush" that saves before
//! showing a cycle's toasts.

use prm_config::{PersistedState, state::STATE_SCHEMA};
use prm_domain::{model::Catalog, notify::NotifierOp};
use tokio::time::Instant;

use super::Service;
use crate::{
    consts::PERSIST_DEBOUNCE,
    errors::UiError,
    jobs::{JobKind, JobOutput, JobTag, Saved, apply_notifier_ops, blocking},
    publish,
};

#[derive(Default)]
pub(super) struct PersistState {
    pub deadline: Option<Instant>,
    in_flight: bool,
    /// Dirtied again while a save was running: save once more afterwards.
    again: bool,
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
        if self.persist.in_flight {
            self.persist.again = true;
            return;
        }
        self.persist.in_flight = true;
        let state = self.build_persisted();
        let repo = self.deps.repo.clone();
        self.spawn(JobTag::global(JobKind::Save), async move {
            let result = blocking(move || repo.save_state(&state)).await;
            JobOutput::Saved {
                what: Saved::State,
                result: save_result(result),
            }
        });
    }

    /// Saves now, then shows the toasts: a crash in between loses a toast, never duplicates one.
    pub(super) fn flush(&mut self, ops: Vec<NotifierOp>) {
        self.persist.deadline = None;
        self.persist.in_flight = true;
        let state = self.build_persisted();
        let repo = self.deps.repo.clone();
        let notifier = self.deps.notifier.clone();
        self.spawn(JobTag::global(JobKind::Flush), async move {
            let saved = save_result(blocking(move || repo.save_state(&state)).await);
            let failures = blocking(move || apply_notifier_ops(notifier.as_ref(), &ops)).await;
            JobOutput::Flushed {
                saved,
                notify_failures: failures.unwrap_or(1),
            }
        });
    }

    /// A state save (plain or flush) finished.
    pub(super) fn on_state_saved(&mut self, result: Result<(), UiError>) {
        self.persist.in_flight = false;
        if let Err(error) = result {
            tracing::warn!(error = %error.title, "state save failed");
            self.notice(crate::event::Notice {
                severity: prm_config::Severity::Warning,
                text: crate::text::STATE_NOT_SAVED.into(),
                org: None,
                action: None,
                ttl: crate::consts::NOTICE_TTL,
            });
        }
        if std::mem::take(&mut self.persist.again) {
            self.persist.deadline = Some(Instant::now());
        }
    }

    pub(super) fn build_persisted(&self) -> PersistedState {
        let orgs = self
            .orgs
            .iter()
            .map(|o| {
                let method = o.link.auth.snapshot().method;
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
        let repo = self.deps.repo.clone();
        self.spawn(JobTag::org(&org, JobKind::Save), async move {
            let result = blocking(move || repo.save_catalog(&org, &catalog)).await;
            JobOutput::Saved {
                what: Saved::Catalog,
                result: save_result(result),
            }
        });
    }
}

pub(super) fn save_result(
    result: Option<Result<(), prm_config::ConfigError>>,
) -> Result<(), UiError> {
    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => Err(UiError::from(&error)),
        None => Err(UiError::internal("save task failed")),
    }
}
