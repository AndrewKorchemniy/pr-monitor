//! Auth in the actor: applying state-machine transitions and their effects, sign-in, silent
//! probes of paused orgs, sign-out and PAT entry.

use std::sync::Arc;

use prm_ado::{AdoError, StaticCredential};
use prm_auth::pat::basic_header;
use prm_domain::{
    OrgId, Person, Secret,
    auth::{AccessToken, AuthError, AuthMethod, SignInMethod},
};

use super::Service;
use crate::{
    auth_machine::{AuthEffect, AuthInput, AuthState, transition},
    consts::{AUTH_PROBE_INTERVAL, PROBE_ON_SHOW_DEBOUNCE},
    errors::UiError,
    event::AppEvent,
    jobs::{self, JobKind, JobOutput, JobTag},
    org::InFlight,
    scheduler::Moment,
    state::{OrgHealth, PauseReason},
};

/// How a paused org waits, by auth state.
enum Pause {
    /// Disabled: nothing is scheduled at all.
    Off,
    Reason(PauseReason),
}

const fn pause_for(state: &AuthState) -> Pause {
    match state {
        AuthState::Disabled => Pause::Off,
        AuthState::SignedOut => Pause::Reason(PauseReason::SignedOut),
        AuthState::Unknown
        | AuthState::Checking
        | AuthState::SignedIn { .. }
        | AuthState::SigningIn { .. }
        | AuthState::NeedsSignIn { .. }
        | AuthState::Blocked { .. } => Pause::Reason(PauseReason::Auth),
    }
}

impl Service {
    pub(super) fn apply_auth(&mut self, idx: usize, input: AuthInput) {
        let now = self.now();
        let org = &mut self.orgs[idx];
        let (next, effects) = transition(&org.live.auth_state, input, now.wall);
        org.live.auth_state = next;
        for effect in effects {
            self.apply_effect(idx, effect, now);
        }
        self.publisher.dirty = true;
    }

    fn apply_effect(&mut self, idx: usize, effect: AuthEffect, now: Moment) {
        match effect {
            AuthEffect::PauseRefresh => self.pause(idx, now),
            AuthEffect::ResumeRefresh { immediate } => self.resume(idx, immediate, now),
            AuthEffect::NotifyProblem => self.add_auth_problem(idx),
            AuthEffect::StartSignIn(method) => self.spawn_sign_in(idx, method),
            AuthEffect::CancelSignIn => self.orgs[idx].link.cancel_sign_in(),
        }
    }

    fn pause(&mut self, idx: usize, now: Moment) {
        let probe_every = self.policy().interval.max(AUTH_PROBE_INTERVAL);
        let live = &mut self.orgs[idx].live;
        let schedule = &mut live.schedule;
        schedule.due = None;
        schedule.due_wall = None;
        match pause_for(&live.auth_state) {
            Pause::Off => {
                schedule.paused = None;
                schedule.next_probe = None;
                live.health = OrgHealth::Disabled;
            }
            Pause::Reason(reason) => {
                schedule.paused = Some(reason);
                schedule.next_probe =
                    (reason == PauseReason::Auth).then_some(now.mono + probe_every);
                live.health = OrgHealth::Paused { reason };
            }
        }
        self.rows.dirty = true;
    }

    fn resume(&mut self, idx: usize, immediate: bool, now: Moment) {
        let interval = self.policy().interval;
        let live = &mut self.orgs[idx].live;
        live.schedule.paused = None;
        live.schedule.next_probe = None;
        if immediate {
            live.schedule.due_now(now);
        } else if live.schedule.due.is_none() {
            live.schedule.due_at(now, interval);
        }
        if matches!(live.health, OrgHealth::Paused { .. } | OrgHealth::Disabled) {
            live.health = OrgHealth::Idle;
        }
        self.rows.dirty = true;
    }

    /// `Command::SignIn`: remembers the state to restore if the user cancels.
    pub(super) fn sign_in(&mut self, org: &OrgId, method: SignInMethod) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let state = &self.orgs[idx].live.auth_state;
        if !matches!(state, AuthState::SigningIn { .. }) {
            self.orgs[idx].link.pre_sign_in = Some(state.clone());
        }
        self.apply_auth(idx, AuthInput::SignInRequested(method));
    }

    fn spawn_sign_in(&mut self, idx: usize, method: SignInMethod) {
        let link = &mut self.orgs[idx].link;
        link.cancel_sign_in();
        let token = self.root.child_token();
        link.sign_in = Some(token.clone());
        let auth = link.auth.clone();
        let org = self.orgs[idx].data.id.clone();
        self.spawn(JobTag::org(&org, JobKind::SignIn), async move {
            let result = auth.sign_in(method, token).await;
            JobOutput::SignIn { org, result }
        });
    }

    pub(super) fn on_sign_in(&mut self, org: &OrgId, result: Result<AuthMethod, AuthError>) {
        let Some(idx) = self.index(org) else {
            return;
        };
        self.orgs[idx].link.sign_in = None;
        self.orgs[idx].live.auth = self.orgs[idx].link.auth.snapshot();
        let event = result.clone().map_err(|e| UiError::from(&e));
        match result {
            Ok(_) => {
                self.orgs[idx].data.signed_out = false;
                self.apply_auth(idx, AuthInput::SignInOk);
                self.mark_persist();
            }
            Err(AuthError::Cancelled) => {
                let restore = self.orgs[idx].link.pre_sign_in.take().unwrap_or_default();
                let restore = Box::new(restore);
                self.apply_auth(idx, AuthInput::SignInCancelled { restore });
            }
            Err(error) => self.apply_auth(idx, AuthInput::SignInFailed(error)),
        }
        self.deps.events.emit(AppEvent::SignInFinished {
            org: org.clone(),
            result: event,
        });
    }

    /// A silent re-check of an org paused for sign-in.
    pub(super) fn start_probe(&mut self, idx: usize) {
        let now = self.now();
        let probe_every = self.policy().interval.max(AUTH_PROBE_INTERVAL);
        let org = &mut self.orgs[idx];
        if org.link.in_flight.is_some() {
            return;
        }
        let cancel = self.root.child_token();
        org.link.in_flight = Some(InFlight {
            cancel: cancel.clone(),
        });
        org.link.last_probe = Some(now.mono);
        org.live.schedule.next_probe = Some(now.mono + probe_every);
        let (auth, id, generation) = (
            org.link.auth.clone(),
            org.data.id.clone(),
            org.link.generation,
        );
        self.spawn(JobTag::org(&id, JobKind::Probe), async move {
            let result = auth.sign_in(SignInMethod::Silent, cancel).await;
            JobOutput::Probe {
                org: id,
                generation,
                result,
            }
        });
    }

    pub(super) fn start_due_probes(&mut self, now: Moment) {
        let due: Vec<usize> = (0..self.orgs.len())
            .filter(|&idx| {
                let org = &self.orgs[idx];
                org.live.schedule.paused == Some(PauseReason::Auth)
                    && org.link.in_flight.is_none()
                    && org
                        .live
                        .schedule
                        .next_probe
                        .is_some_and(|at| at <= now.mono)
            })
            .collect();
        for idx in due {
            self.start_probe(idx);
        }
    }

    /// The window was shown: re-check paused orgs (debounced), so a fixed sign-in shows quickly.
    pub(super) fn probe_on_show(&mut self) {
        let now = self.now();
        let due: Vec<usize> = (0..self.orgs.len())
            .filter(|&idx| {
                let org = &self.orgs[idx];
                org.live.schedule.paused == Some(PauseReason::Auth)
                    && org.link.in_flight.is_none()
                    && org
                        .link
                        .last_probe
                        .is_none_or(|at| now.mono >= at + PROBE_ON_SHOW_DEBOUNCE)
            })
            .collect();
        for idx in due {
            self.start_probe(idx);
        }
    }

    pub(super) fn on_probe(
        &mut self,
        org: &OrgId,
        generation: u64,
        result: Result<AuthMethod, AuthError>,
    ) {
        let Some(idx) = self.index(org) else {
            return;
        };
        if self.orgs[idx].link.generation != generation {
            return;
        }
        self.orgs[idx].link.in_flight = None;
        self.orgs[idx].live.auth = self.orgs[idx].link.auth.snapshot();
        let input = match result {
            Ok(method) => AuthInput::AuthOk { method },
            Err(error) => AuthInput::AuthFailed(error),
        };
        self.apply_auth(idx, input);
    }

    /// `Command::SignOut`: forget secrets, stop refreshing, keep the rows (shown as stale).
    pub(super) fn sign_out(&mut self, org: &OrgId) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let runtime = &mut self.orgs[idx];
        if runtime.link.cancel_in_flight() {
            self.progress.remove(org);
            self.leave_cycle(idx);
        }
        let runtime = &mut self.orgs[idx];
        runtime.data.signed_out = true;
        runtime.data.me = None;
        let auth = runtime.link.auth.clone();
        let id = org.clone();
        self.spawn(JobTag::org(org, JobKind::SignOut), async move {
            let result = auth.sign_out().await;
            JobOutput::SignedOut { org: id, result }
        });
        self.apply_auth(idx, AuthInput::SignOutRequested);
        self.mark_persist();
    }

    /// `Command::SetPat`: checks the PAT with a one-off client before storing it.
    pub(super) fn set_pat(&mut self, org: &OrgId, pat: Secret) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let token = AccessToken {
            header: basic_header(&pat),
            expires_at: None,
            method: AuthMethod::Pat,
            id: pat.id(),
        };
        let link = &self.orgs[idx].link;
        let check = self.deps.ado.connect(
            link.endpoint.clone(),
            Arc::new(StaticCredential(token)),
            self.connectors_policy(),
        );
        let auth = link.auth.clone();
        let id = org.clone();
        self.spawn(JobTag::org(org, JobKind::SetPat), async move {
            let result = jobs::set_pat(check, auth, pat).await;
            JobOutput::SetPat { org: id, result }
        });
    }

    pub(super) fn on_set_pat(&mut self, org: &OrgId, result: Result<Person, AdoError>) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let event = result.clone().map_err(|e| UiError::from(&e));
        if let Ok(me) = result {
            let data = &mut self.orgs[idx].data;
            data.me = Some(me);
            data.signed_out = false;
            self.orgs[idx].live.auth = self.orgs[idx].link.auth.snapshot();
            self.apply_auth(idx, AuthInput::PatStored);
            self.mark_persist();
        }
        self.deps.events.emit(AppEvent::PatResult {
            org: org.clone(),
            result: event,
        });
    }
}
