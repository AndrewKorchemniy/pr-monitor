//! `ApplyConfig`: normalize, save, and restart only what changed.

use std::sync::Arc;

use prm_config::{Config, OrgChanges, OrgProfile, Severity, diff_configs, normalize};
use prm_domain::OrgId;

use super::{Service, persist::save_result};
use crate::{
    auth_machine::AuthInput,
    consts::NOTICE_TTL,
    event::{AppEvent, Notice},
    jobs::{JobKind, JobOutput, JobTag, Saved, blocking},
    org::{Connectors, OrgData, OrgRuntime},
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
            self.notice(Notice {
                severity: Severity::Info,
                text: Arc::from(text::NETWORK_RESTART),
                org: None,
                action: None,
                ttl: NOTICE_TTL,
            });
        }
        if changes.autostart {
            self.spawn_autostart(Some(self.config.ui.launch_at_login));
        }
        self.rows.dirty = true;
    }

    fn spawn_config_save(&mut self) {
        let config = (*self.config).clone();
        let repo = self.deps.repo.clone();
        self.spawn(JobTag::global(JobKind::Save), async move {
            let result = blocking(move || repo.save_config(&config)).await;
            JobOutput::Saved {
                what: Saved::Config,
                result: save_result(result),
            }
        });
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
        link.cancel_sign_in();
        link.abort_search();
        self.leave_cycle(idx);
        self.progress.remove(org);
        self.seen.retain(|key, _| &key.org != org);
        let (repo, id) = (self.deps.repo.clone(), org.clone());
        self.spawn(JobTag::org(org, JobKind::Save), async move {
            let result = blocking(move || repo.delete_org_data(&id)).await;
            JobOutput::Saved {
                what: Saved::OrgDataDeleted,
                result: save_result(result),
            }
        });
    }

    /// Orders runtimes like the config, updates profiles, and creates runtimes for new orgs
    /// (due now). Runtimes of removed orgs are dropped.
    fn rebuild_org_list(&mut self) {
        let now = self.now();
        let mut old = std::mem::take(&mut self.orgs);
        let profiles = self.config.orgs.clone();
        for profile in profiles {
            let Some(id) = profile.org_id().cloned() else {
                continue;
            };
            if self.index(&id).is_some() {
                continue;
            }
            let runtime = self.reuse_or_create(&mut old, id, profile, now);
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
        let now = self.now();
        if change.endpoint || change.auth {
            self.restart_jobs(idx);
            self.orgs[idx].link.cancel_sign_in();
            self.reconnect(idx);
            self.apply_auth(idx, AuthInput::Reset);
        } else if change.scope || change.authors {
            self.restart_jobs(idx);
            let runtime = &mut self.orgs[idx];
            runtime.data.rebaseline_pending = true;
            runtime.live.schedule.due_now(now);
        }
        if change.classify {
            self.orgs[idx].data.reclassify();
        }
        if change.enabled {
            self.apply_enabled(idx);
        }
    }

    /// Cancels the running refresh or probe; its result is ignored when it arrives.
    fn restart_jobs(&mut self, idx: usize) {
        if self.orgs[idx].link.cancel_in_flight() {
            let org = self.orgs[idx].data.id.clone();
            self.progress.remove(&org);
            self.leave_cycle(idx);
        }
    }

    fn reconnect(&mut self, idx: usize) {
        let policy = self.connectors_policy();
        let connectors = Connectors {
            auth: self.deps.auth.as_ref(),
            ado: self.deps.ado.as_ref(),
            policy: &policy,
        };
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
        if self.orgs[idx].data.signed_out {
            self.apply_auth(idx, AuthInput::SignOutRequested);
        }
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
