//! `ApplyConfig`: normalize, save, and restart only what changed.

use std::sync::Arc;

use prm_config::{Config, OrgChanges, OrgProfile, Severity, diff_configs, normalize};
use prm_domain::OrgId;

use super::Service;
use crate::{
    auth_machine::AuthInput,
    event::AppEvent,
    jobs::{JobKind, JobTag, Saved},
    org::{OrgData, OrgRuntime, org_profiles},
    scheduler::{Moment, jittered},
    text,
};

impl Service {
    pub(super) fn apply_config(&mut self, new: Config) {
        let (config, issues) = normalize(new);
        let changes = diff_configs(&self.config, &config);
        self.config = Arc::new(config);
        self.config_issues = issues.into();
        self.autostart.desired = self.config.ui.launch_at_login;
        self.spawn_config_save();
        for org in &changes.removed {
            self.remove_org(org);
        }
        self.rebuild_org_list();
        for (org, change) in &changes.changed {
            self.apply_org_changes(org, *change);
        }
        if changes.refresh {
            self.reschedule_all();
        }
        if changes.network {
            self.global_notice(Severity::Info, text::NETWORK_RESTART);
        }
        if changes.autostart {
            self.spawn_autostart(Some(self.config.ui.launch_at_login));
        }
        self.rows.dirty = true;
    }

    fn spawn_config_save(&mut self) {
        let config = (*self.config).clone();
        let tag = JobTag::global(JobKind::Save);
        self.spawn_save(tag, Saved::Config, move |repo| repo.save_config(&config));
    }

    /// The config save finished: tell the settings screen.
    pub(super) fn on_config_saved(&mut self, saved: bool) {
        self.deps.events.emit(AppEvent::ConfigApplied {
            issues: self.config_issues.clone(),
            saved,
        });
    }

    /// Stops the org's jobs and forgets its data; its secrets stay unless the user signed out.
    fn remove_org(&mut self, org: &OrgId) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let link = &mut self.orgs[idx].link;
        link.cancel_in_flight();
        link.abandon_sign_in();
        link.abort_search();
        self.leave_cycle(idx);
        self.progress.remove(org);
        self.seen.retain(|key, _| &key.org != org);
        let id = org.clone();
        let tag = JobTag::org(org, JobKind::Save);
        self.spawn_save(tag, Saved::OrgDataDeleted, move |repo| {
            repo.delete_org_data(&id)
        });
    }

    /// Orders runtimes like the config, updates profiles, and creates runtimes for new orgs
    /// (due now). Runtimes of removed orgs are dropped.
    fn rebuild_org_list(&mut self) {
        let now = self.now();
        let mut old = std::mem::take(&mut self.orgs);
        let config = self.config.clone();
        for (id, profile) in org_profiles(&config) {
            let runtime = self.reuse_or_create(&mut old, id, profile.clone(), now);
            self.orgs.push(runtime);
        }
    }

    /// The existing runtime with the new profile, or a new runtime that is due now.
    fn reuse_or_create(
        &self,
        old: &mut Vec<OrgRuntime>,
        id: OrgId,
        profile: OrgProfile,
        now: Moment,
    ) -> OrgRuntime {
        if let Some(pos) = old.iter().position(|o| o.data.id == id) {
            let mut runtime = old.swap_remove(pos);
            runtime.data.set_profile(profile, now.wall);
            return runtime;
        }
        let data = OrgData::new(id, profile, None, None, now.wall);
        let mut runtime = self.new_runtime(data);
        if runtime.data.profile.enabled {
            runtime.live.schedule.due_now(now);
        }
        runtime
    }

    fn apply_org_changes(&mut self, org: &OrgId, change: OrgChanges) {
        let Some(idx) = self.index(org) else {
            return;
        };
        if change.scope || change.authors {
            // why: PRs that newly come into view are not news; the next refresh is silent.
            self.orgs[idx].data.rebaseline_pending = true;
        }
        if change.endpoint || change.auth {
            self.reset_auth(idx);
        } else if change.scope || change.authors {
            self.restart_jobs(idx);
            self.refresh_soon(idx);
        }
        if change.classify {
            self.orgs[idx].data.reclassify();
        }
        if change.enabled {
            self.apply_enabled(idx);
        }
    }

    /// New auth settings: new clients, a fresh auth state, and a refresh (unless the org is
    /// disabled or signed out).
    fn reset_auth(&mut self, idx: usize) {
        self.restart_jobs(idx);
        self.abandon_sign_in(idx);
        self.reconnect(idx);
        // why: the new settings may sign in as someone else; the next refresh asks.
        self.orgs[idx].data.me = None;
        self.apply_auth(idx, AuthInput::Reset);
        self.keep_signed_out(idx);
    }

    /// Refreshes the org now if it is enabled and not paused.
    fn refresh_soon(&mut self, idx: usize) {
        let now = self.now();
        let runtime = &mut self.orgs[idx];
        if runtime.data.profile.enabled && runtime.live.schedule.paused.is_none() {
            runtime.live.schedule.due_now(now);
        }
    }

    /// Cancels the running refresh or probe; its result is ignored when it arrives.
    pub(super) fn restart_jobs(&mut self, idx: usize) {
        if self.orgs[idx].link.cancel_in_flight() {
            let org = self.orgs[idx].data.id.clone();
            self.progress.remove(&org);
            self.leave_cycle(idx);
        }
    }

    fn reconnect(&mut self, idx: usize) {
        let policy = self.connectors_policy();
        let connectors = self.deps.connectors(&policy);
        let OrgRuntime { data, link, .. } = &mut self.orgs[idx];
        link.reconnect(data, &connectors);
    }

    fn apply_enabled(&mut self, idx: usize) {
        if !self.orgs[idx].data.profile.enabled {
            self.restart_jobs(idx);
            self.apply_auth(idx, AuthInput::Disabled);
            return;
        }
        self.apply_auth(idx, AuthInput::Enabled);
        self.keep_signed_out(idx);
    }

    /// The interval changed: no org waits longer than the new interval.
    fn reschedule_all(&mut self) {
        let now = self.now();
        let policy = self.policy();
        for org in &mut self.orgs {
            let schedule = &mut org.live.schedule;
            if schedule.due.is_some() {
                let base = policy.interval.saturating_mul(schedule.multiplier);
                let delay = jittered(base, policy.jitter_pct, &mut self.rng);
                schedule.due_no_later_than(now, delay);
            }
        }
    }
}
