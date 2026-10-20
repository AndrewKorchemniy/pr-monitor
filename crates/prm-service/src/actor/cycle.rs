//! Notification cycles: refreshes started together are toasted together, once all of them
//! finish. State is persisted before any toast is shown.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use prm_domain::{
    OrgId,
    diff::ChangeEvent,
    notify::{AuthProblem, PolicyInput, plan_notifications},
};

use super::Service;
use crate::state::AuthStateView;

#[derive(Default)]
pub(super) struct Cycle {
    pending: BTreeSet<OrgId>,
    events: Vec<ChangeEvent>,
    auth_problems: Vec<AuthProblem>,
}

#[derive(Default)]
pub(super) struct Cycles {
    open: BTreeMap<u64, Cycle>,
    next_id: u64,
}

impl Service {
    /// Starts refreshes for these orgs as one notification cycle.
    pub(super) fn start_cycle(&mut self, orgs: &[OrgId]) {
        let id = self.cycles.next_id;
        self.cycles.next_id += 1;
        let mut cycle = Cycle::default();
        for org in orgs {
            if let Some(idx) = self.index(org)
                && self.start_refresh(idx, id)
            {
                cycle.pending.insert(org.clone());
            }
        }
        if !cycle.pending.is_empty() {
            tracing::debug!(cycle = id, orgs = cycle.pending.len(), "cycle started");
            self.cycles.open.insert(id, cycle);
        }
    }

    /// The org's refresh is over; the cycle finishes when no org is pending.
    pub(super) fn leave_cycle(&mut self, idx: usize) {
        let Some(id) = self.orgs[idx].link.cycle.take() else {
            return;
        };
        let org = &self.orgs[idx].data.id;
        let Some(cycle) = self.cycles.open.get_mut(&id) else {
            return;
        };
        cycle.pending.remove(org);
        if cycle.pending.is_empty()
            && let Some(cycle) = self.cycles.open.remove(&id)
        {
            self.finish_cycle(&cycle);
        }
    }

    pub(super) fn add_events(&mut self, idx: usize, events: Vec<ChangeEvent>) {
        match self.current_cycle(idx) {
            Some(cycle) => cycle.events.extend(events),
            None => self.finish_cycle(&Cycle {
                events,
                ..Cycle::default()
            }),
        }
    }

    /// The org newly needs sign-in: one toast, merged into the running cycle if there is one.
    pub(super) fn add_auth_problem(&mut self, idx: usize) {
        let data = &self.orgs[idx].data;
        let message = match self.orgs[idx].live.auth_state.view() {
            AuthStateView::NeedsSignIn { error } | AuthStateView::Blocked { error } => error.title,
            AuthStateView::Unknown
            | AuthStateView::Checking
            | AuthStateView::SignedIn
            | AuthStateView::SigningIn { .. }
            | AuthStateView::SignedOut
            | AuthStateView::Disabled => return,
        };
        let problem = AuthProblem {
            org: data.id.clone(),
            org_label: data.label.clone(),
            message,
        };
        match self.current_cycle(idx) {
            Some(cycle) => cycle.auth_problems.push(problem),
            None => self.finish_cycle(&Cycle {
                auth_problems: vec![problem],
                ..Cycle::default()
            }),
        }
    }

    fn current_cycle(&mut self, idx: usize) -> Option<&mut Cycle> {
        let id = self.orgs[idx].link.cycle?;
        self.cycles.open.get_mut(&id)
    }

    fn finish_cycle(&mut self, cycle: &Cycle) {
        let now = self.now();
        let muted: HashSet<OrgId> = self
            .orgs
            .iter()
            .filter(|o| o.data.profile.muted)
            .map(|o| o.data.id.clone())
            .collect();
        let labels = self
            .orgs
            .iter()
            .map(|o| (o.data.id.clone(), o.data.label.clone()))
            .collect();
        let plan = plan_notifications(&PolicyInput {
            prefs: &self.config.notifications,
            events: &cycle.events,
            auth_problems: &cycle.auth_problems,
            paused: self.paused_until.is_some_and(|until| until > now.wall),
            ui_focused: self.presence.visible && self.presence.focused,
            muted_orgs: &muted,
            multi_org: self.config.enabled_orgs().count() > 1,
            org_labels: &labels,
        });
        self.last_suppressed = plan.suppressed;
        tracing::debug!(
            ops = plan.ops.len(),
            suppressed = plan.suppressed,
            "cycle finished"
        );
        if !plan.ops.is_empty() {
            self.flush(plan.ops);
        }
    }
}
