//! Finished jobs: routes each output to its handler, and attributes panics.

use std::sync::Arc;

use prm_ado::AdoError;
use prm_config::Severity;
use prm_domain::{OrgId, auth::AuthError, model::Catalog};
use tokio::task::Id;

use super::Service;
use crate::{
    errors::UiError,
    event::AppEvent,
    jobs::{JobKind, JobOutput, JobTag, Saved},
    text,
};

impl Service {
    pub(super) fn on_output(&mut self, output: JobOutput) {
        match output {
            JobOutput::Refresh {
                org,
                generation,
                result,
            } => self.on_refresh(&org, generation, result),
            JobOutput::Probe {
                org,
                generation,
                result,
            } => self.on_probe(&org, generation, result),
            JobOutput::SignIn { org, seq, result } => self.on_sign_in(&org, seq, result),
            JobOutput::SignedOut { org, result } => self.on_signed_out(&org, result),
            JobOutput::SetPat { org, result } => self.on_set_pat(&org, result),
            JobOutput::Catalog {
                org,
                generation,
                result,
            } => self.on_catalog(&org, generation, result),
            JobOutput::Search {
                org,
                request,
                query,
                result,
            } => self.on_search(&org, request, &query, result),
            JobOutput::Resolve {
                org,
                request,
                candidate,
                result,
            } => self.on_resolve(&org, request, &candidate, result),
            JobOutput::Saved { what, result } => self.on_saved(what, result),
            JobOutput::Flushed {
                saved,
                notify_failures,
            } => self.on_flushed(saved, notify_failures),
            JobOutput::Capability(capability) => self.capability = capability,
            JobOutput::Autostart {
                seq,
                state,
                set_error,
            } => self.on_autostart(seq, state, set_error.as_ref()),
            JobOutput::Opened(result) => self.on_opened(result),
            JobOutput::TestNotification(result) => {
                let result = result.map_err(|e| UiError::port(text::NOTIFICATION_FAILED, &e));
                self.deps
                    .events
                    .emit(AppEvent::NotificationTestResult(result));
            }
        }
    }

    fn on_flushed(&mut self, saved: Result<(), UiError>, notify_failures: usize) {
        if notify_failures > 0 {
            tracing::warn!(notify_failures, "some notifications failed");
        }
        self.on_state_saved(saved);
    }

    fn on_opened(&mut self, result: Result<(), prm_domain::ports::PortError>) {
        if let Err(error) = result {
            tracing::warn!(error = %error.message, "open failed");
            self.global_notice(Severity::Warning, text::OPEN_FAILED);
        }
    }

    fn on_saved(&mut self, what: Saved, result: Result<(), UiError>) {
        match what {
            Saved::State => self.on_state_saved(result),
            Saved::Config => {
                if let Err(error) = &result {
                    tracing::warn!(error = %error.title, "config save failed");
                    self.global_notice(Severity::Error, text::SETTINGS_NOT_SAVED);
                }
                self.on_config_saved(result.is_ok());
            }
            Saved::Catalog | Saved::OrgDataDeleted => {
                if let Err(error) = result {
                    tracing::warn!(what = ?what, error = %error.title, "save failed");
                }
            }
        }
    }

    fn on_signed_out(&mut self, org: &OrgId, result: Result<(), AuthError>) {
        if let Some(idx) = self.index(org) {
            self.orgs[idx].live.auth = self.orgs[idx].link.auth.snapshot();
        }
        if let Err(error) = result {
            tracing::warn!(org = %org, %error, "sign-out could not delete secrets");
            self.org_notice(org, &error.to_string());
        }
    }

    fn on_catalog(&mut self, org: &OrgId, generation: u64, result: Result<Catalog, AdoError>) {
        let Some(idx) = self.index(org) else {
            return;
        };
        self.orgs[idx].link.catalog_loading = false;
        if self.orgs[idx].link.generation != generation {
            return;
        }
        match result {
            Ok(catalog) => {
                self.save_catalog(idx, &catalog);
                self.orgs[idx].data.catalog = Some(Arc::new(catalog));
            }
            Err(error) => {
                tracing::warn!(org = %org, %error, "catalog failed");
                self.org_notice(org, text::CATALOG_FAILED);
            }
        }
    }

    /// Applies only the newest autostart job's result; an older read may finish later.
    fn on_autostart(
        &mut self,
        seq: u64,
        state: Result<crate::ports::AutostartState, prm_domain::ports::PortError>,
        set_error: Option<&prm_domain::ports::PortError>,
    ) {
        if seq != self.autostart_seq {
            return;
        }
        if set_error.is_some() {
            self.global_notice(Severity::Warning, text::AUTOSTART_FAILED);
        }
        let read_error = state.as_ref().err().map(|e| e.message.clone());
        self.autostart.actual = state.ok();
        self.autostart.error = set_error.map(|e| e.message.clone()).or(read_error);
    }

    pub(super) fn on_panic(&mut self, job: Id, tag: Option<JobTag>) {
        let Some(tag) = tag else {
            return;
        };
        tracing::error!(job = ?tag.kind, org = ?tag.org, "background job panicked");
        let idx = tag.org.as_ref().and_then(|org| self.index(org));
        match (tag.kind, idx) {
            (JobKind::Refresh | JobKind::Probe, Some(idx)) => self.org_job_panicked(idx),
            (JobKind::SignIn, Some(idx)) => {
                let org = self.orgs[idx].data.id.clone();
                let seq = self.orgs[idx].link.sign_in_seq;
                let error = AuthError::Other("sign-in task failed".into());
                self.on_sign_in(&org, seq, Err(error));
            }
            (JobKind::Catalog, Some(idx)) => self.orgs[idx].link.catalog_loading = false,
            (JobKind::Save | JobKind::Flush, _) => self.on_save_panicked(job),
            (
                JobKind::Refresh
                | JobKind::Probe
                | JobKind::SignIn
                | JobKind::Catalog
                | JobKind::SignOut
                | JobKind::SetPat
                | JobKind::Search
                | JobKind::Resolve
                | JobKind::Port,
                _,
            ) => {}
        }
    }
}
