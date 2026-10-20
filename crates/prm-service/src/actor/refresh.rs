//! Starting refreshes (scheduled, manual, after resume) and applying their results.

use std::sync::Arc;

use prm_domain::{
    OrgId,
    auth::{AuthError, AuthSeverity},
};

use super::Service;
use crate::{
    auth_machine::AuthInput,
    consts::{MANUAL_REFRESH_DEBOUNCE, RESUME_SETTLE},
    errors::UiError,
    event::AppEvent,
    jobs::{
        JobKind, JobOutput, JobTag, RefreshFailure, RefreshOutput,
        refresh::{self, RefreshCtx},
    },
    org::InFlight,
    scheduler::{DueCheck, FailureKind, Moment, after_failure, after_success, due_orgs},
    state::{OrgHealth, PauseReason},
    text,
};

impl Service {
    pub(super) fn start_due_refreshes(&mut self, now: Moment) {
        let due = due_orgs(
            self.orgs.iter().map(|o| DueCheck {
                org: &o.data.id,
                schedule: &o.live.schedule,
                in_flight: o.link.in_flight.is_some(),
            }),
            now.mono,
        );
        if !due.is_empty() {
            self.start_cycle(&due);
            self.publisher.dirty = true;
        }
    }

    /// The machine slept: overdue orgs refresh shortly, once the network is back.
    pub(super) fn settle_after_resume(&mut self, now: Moment) {
        tracing::info!("resume detected");
        for org in &mut self.orgs {
            let schedule = &mut org.live.schedule;
            if schedule.due_wall.is_some_and(|due| due <= now.wall) {
                schedule.due_at(now, RESUME_SETTLE);
            }
        }
    }

    /// `RefreshNow` / `RefreshOrg`: debounced per org; paused orgs are probed instead.
    pub(super) fn refresh_manual(&mut self, target: Option<&OrgId>) {
        let now = self.now();
        let mut start = Vec::new();
        let mut probe = Vec::new();
        for (idx, org) in self.orgs.iter_mut().enumerate() {
            let wanted = target.is_none_or(|t| t == &org.data.id);
            let recent = org
                .link
                .last_manual
                .is_some_and(|at| now.mono < at + MANUAL_REFRESH_DEBOUNCE);
            if !wanted || !org.data.profile.enabled || recent {
                continue;
            }
            org.link.last_manual = Some(now.mono);
            match org.live.schedule.paused {
                Some(PauseReason::Auth) => probe.push(idx),
                Some(PauseReason::SignedOut | PauseReason::RateLimited) => {}
                None if org.link.in_flight.is_none() => {
                    org.live.schedule.failures = 0;
                    start.push(org.data.id.clone());
                }
                None => {}
            }
        }
        for idx in probe {
            self.start_probe(idx);
        }
        if !start.is_empty() {
            self.start_cycle(&start);
        }
    }

    /// Starts one org's refresh as part of `cycle`. `false` if it can't run now.
    pub(super) fn start_refresh(&mut self, idx: usize, cycle: u64) -> bool {
        let now = self.now();
        let org = &mut self.orgs[idx];
        if org.link.in_flight.is_some() || !org.data.profile.enabled {
            return false;
        }
        let ctx = RefreshCtx {
            org: org.data.id.clone(),
            api: org.link.api.clone(),
            auth: org.link.auth.clone(),
            profile: org.data.profile.clone(),
            me: org.data.me.clone(),
            catalog: org.data.catalog.clone(),
            clock: self.deps.clock.clone(),
            prev: org.data.records.clone(),
            org_wide: org.data.org_wide,
            progress: self.progress_tx.clone(),
        };
        let cancel = self.root.child_token();
        org.link.in_flight = Some(InFlight {
            cancel: cancel.clone(),
        });
        org.link.cycle = Some(cycle);
        org.live.schedule.due = None;
        org.live.schedule.due_wall = None;
        org.live.health = OrgHealth::Refreshing { since: now.wall };
        let (id, generation) = (org.data.id.clone(), org.link.generation);
        self.apply_auth(idx, AuthInput::RefreshStarted);
        self.spawn(JobTag::org(&id, JobKind::Refresh), async move {
            let result = refresh::run(ctx, cancel).await;
            JobOutput::Refresh {
                org: id,
                generation,
                result,
            }
        });
        true
    }

    pub(super) fn on_refresh(
        &mut self,
        org: &OrgId,
        generation: u64,
        result: Result<Box<RefreshOutput>, RefreshFailure>,
    ) {
        let Some(idx) = self.index(org) else {
            return;
        };
        if self.orgs[idx].link.generation != generation {
            return;
        }
        self.orgs[idx].link.in_flight = None;
        self.progress.remove(org);
        match result {
            Ok(output) => self.refresh_succeeded(idx, *output),
            Err(RefreshFailure::Auth(error)) => self.refresh_auth_failed(idx, error),
            Err(RefreshFailure::Ado(error)) => {
                self.refresh_failed(idx, FailureKind::of(&error), UiError::from(&error));
            }
            Err(RefreshFailure::Timeout) => {
                let error = UiError::new(text::REFRESH_TIMED_OUT);
                self.refresh_failed(idx, FailureKind::Other, error);
            }
            Err(RefreshFailure::Cancelled) => {}
        }
        self.leave_cycle(idx);
    }

    fn refresh_succeeded(&mut self, idx: usize, output: RefreshOutput) {
        let now = self.now();
        let policy = self.policy();
        let RefreshOutput {
            fetched,
            auth,
            rate,
            requests,
            elapsed,
        } = output;
        let refetched = fetched.catalog.clone();
        let failed = fetched.failed_queries.len();
        let org = &mut self.orgs[idx];
        tracing::debug!(org = %org.data.id, requests, elapsed_ms = elapsed.as_millis(), "refreshed");
        let absorbed = org.data.absorb(fetched, now.wall);
        org.live.health = if failed == 0 {
            OrgHealth::Ok
        } else {
            OrgHealth::Degraded {
                failed_queries: u32::try_from(failed).unwrap_or(u32::MAX),
                detail: Arc::from(text::degraded(failed)),
            }
        };
        org.live.last_error = None;
        after_success(
            policy,
            &mut org.live.schedule,
            now,
            rate.headroom(),
            &mut self.rng,
        );
        org.live.rate = rate;
        org.live.auth = auth;
        if absorbed.first_baseline {
            let snapshot = &org.data.snapshot;
            self.seen
                .extend(snapshot.iter().map(|(k, c)| (k.clone(), c.fingerprint)));
        }
        if let Some(catalog) = refetched {
            self.save_catalog(idx, &catalog);
        }
        if let Some(method) = auth.method {
            self.apply_auth(idx, AuthInput::AuthOk { method });
        }
        self.rows.dirty = true;
        self.mark_persist();
        if !absorbed.events.is_empty() {
            let events: Arc<[_]> = absorbed.events.clone().into();
            self.deps.events.emit(AppEvent::Changes(events));
            self.add_events(idx, absorbed.events);
        }
    }

    fn refresh_auth_failed(&mut self, idx: usize, error: AuthError) {
        let transient = error.severity() == AuthSeverity::Transient;
        let ui = UiError::from(&error);
        self.orgs[idx].live.auth = self.orgs[idx].link.auth.snapshot();
        self.apply_auth(idx, AuthInput::AuthFailed(error));
        if transient {
            self.refresh_failed(idx, FailureKind::Other, ui);
        } else {
            self.orgs[idx].live.last_error = Some(ui);
            self.rows.dirty = true;
        }
    }

    pub(super) fn refresh_failed(&mut self, idx: usize, failure: FailureKind, error: UiError) {
        let now = self.now();
        let policy = self.policy();
        let org = &mut self.orgs[idx];
        after_failure(policy, &mut org.live.schedule, now, failure, &mut self.rng);
        tracing::warn!(org = %org.data.id, error = %error.title, "refresh failed");
        org.live.health = match failure {
            FailureKind::RateLimited { .. } => OrgHealth::Paused {
                reason: PauseReason::RateLimited,
            },
            FailureKind::Other => OrgHealth::Failed {
                error: error.clone(),
                since: now.wall,
                retry_at: org.live.schedule.due_wall,
            },
        };
        org.live.last_error = Some(error);
        self.rows.dirty = true;
    }

    /// A refresh or probe job panicked: report it like a failure.
    pub(super) fn org_job_panicked(&mut self, idx: usize) {
        self.orgs[idx].link.in_flight = None;
        self.progress.remove(&self.orgs[idx].data.id.clone());
        let error = UiError::internal("refresh task panicked");
        self.refresh_failed(idx, FailureKind::Other, error);
        self.leave_cycle(idx);
    }
}
