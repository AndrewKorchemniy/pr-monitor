//! Command dispatch and the small command handlers (seen marks, links, presence, ports).

use std::{ops::ControlFlow, sync::Arc};

use prm_domain::{OrgId, PrKey, app::is_openable_url, notify::Notification, uri::AppUri};

use super::Service;
use crate::{
    command::Command,
    event::AppEvent,
    jobs::{JobKind, JobOutput, JobTag, blocking},
    publish,
};

impl Service {
    pub(super) fn on_command(&mut self, command: Command) -> ControlFlow<()> {
        self.publisher.dirty = true;
        match command {
            Command::RefreshNow => self.refresh_manual(None),
            Command::RefreshOrg(org) => self.refresh_manual(Some(&org)),
            Command::LoadCatalog { org, force } => self.load_catalog(&org, force),
            Command::ApplyConfig(config) => self.apply_config(*config),
            Command::SignIn { org, method } => self.sign_in(&org, method),
            Command::CancelSignIn { org } => self.cancel_sign_in(&org),
            Command::SignOut { org } => self.sign_out(&org),
            Command::SetPat { org, pat } => self.set_pat(&org, pat),
            Command::SearchPeople {
                org,
                query,
                request,
            } => self.search_people(&org, &query, request),
            Command::ResolvePerson {
                org,
                candidate,
                request,
            } => self.resolve_person(&org, candidate, request),
            Command::OpenPr(key) => self.open_pr(&key),
            Command::OpenUrl(url) => {
                self.open_url(url);
            }
            Command::MarkSeen(keys) => self.mark_seen(&keys),
            Command::MarkAllSeen => self.mark_all_seen(),
            Command::SaveUiState(state) => {
                self.ui_state = Arc::new(*state);
                self.mark_persist();
            }
            Command::UiPresence { visible, focused } => self.presence_changed(visible, focused),
            Command::SetNotificationsPaused { until } => {
                self.paused_until = until;
                self.mark_persist();
            }
            Command::TestNotification => self.test_notification(),
            Command::Activate(uri) => self.activate(uri),
            Command::Shutdown => return ControlFlow::Break(()),
        }
        ControlFlow::Continue(())
    }

    fn cancel_sign_in(&mut self, org: &OrgId) {
        if let Some(idx) = self.index(org) {
            self.orgs[idx].link.cancel_sign_in();
        }
    }

    fn activate(&mut self, uri: AppUri) {
        match uri {
            AppUri::Refresh => self.refresh_manual(None),
            AppUri::Show { .. } => self.deps.events.emit(AppEvent::Activate(uri)),
        }
    }

    fn open_pr(&mut self, key: &PrKey) {
        let url = self
            .rows
            .rows
            .iter()
            .find(|r| &r.key == key)
            .and_then(|r| url::Url::parse(&r.web_url).ok());
        let Some(url) = url else {
            return;
        };
        if self.open_url(url) {
            self.mark_seen(std::slice::from_ref(key));
        }
    }

    /// Opens an allow-listed link; `false` (and logged) for anything else.
    fn open_url(&mut self, url: url::Url) -> bool {
        if !is_openable_url(&url) {
            tracing::warn!(host = url.host_str(), "blocked link");
            return false;
        }
        let opener = self.deps.opener.clone();
        self.spawn(JobTag::global(JobKind::Port), async move {
            let result = blocking(move || opener.open(&url)).await;
            JobOutput::Opened(result.unwrap_or_else(port_panicked))
        });
        true
    }

    fn mark_seen(&mut self, keys: &[PrKey]) {
        let orgs: Vec<_> = self.orgs.iter().map(|o| &o.data).collect();
        let marks = publish::fingerprints(keys, &orgs);
        self.seen.extend(marks);
        self.rows.dirty = true;
        self.mark_persist();
    }

    fn mark_all_seen(&mut self) {
        let marks = self
            .orgs
            .iter()
            .flat_map(|o| o.data.snapshot.iter())
            .map(|(key, classified)| (key.clone(), classified.fingerprint));
        self.seen.extend(marks);
        self.rows.dirty = true;
        self.mark_persist();
    }

    fn presence_changed(&mut self, visible: bool, focused: bool) {
        let shown = visible && !self.presence.visible;
        self.presence = super::Presence { visible, focused };
        if shown {
            self.spawn_capability_check();
            self.probe_on_show();
        }
    }

    fn test_notification(&mut self) {
        let notifier = self.deps.notifier.clone();
        self.spawn(JobTag::global(JobKind::Port), async move {
            let result = blocking(move || notifier.show(&Notification::test())).await;
            JobOutput::TestNotification(result.unwrap_or_else(port_panicked))
        });
    }

    fn load_catalog(&mut self, org: &OrgId, force: bool) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let now = self.deps.clock.now();
        let runtime = &mut self.orgs[idx];
        let fresh = runtime
            .data
            .catalog
            .as_ref()
            .is_some_and(|c| c.is_fresh(now, crate::consts::CATALOG_TTL));
        if (fresh && !force) || runtime.link.catalog_loading {
            return;
        }
        runtime.link.catalog_loading = true;
        runtime.live.catalog_error = None;
        let (api, id, generation) = (
            runtime.link.api.clone(),
            org.clone(),
            runtime.link.generation,
        );
        self.spawn(JobTag::org(org, JobKind::Catalog), async move {
            let result = crate::jobs::refresh::fetch_catalog(api.as_ref(), now).await;
            JobOutput::Catalog {
                org: id,
                generation,
                result,
            }
        });
    }

    /// Reads launch-at-login (after setting it, when `desired` is given).
    pub(super) fn spawn_autostart(&mut self, desired: Option<bool>) {
        self.autostart_seq += 1;
        let seq = self.autostart_seq;
        let autostart = self.deps.autostart.clone();
        self.spawn(JobTag::global(JobKind::Port), async move {
            let outcome = blocking(move || {
                let set_error = desired.and_then(|on| autostart.set_enabled(on).err());
                (autostart.state(), set_error)
            })
            .await;
            let (state, set_error) = outcome.unwrap_or_else(|| (Err(port_panic_error()), None));
            JobOutput::Autostart {
                seq,
                state,
                set_error,
            }
        });
    }
}

fn port_panic_error() -> prm_domain::ports::PortError {
    prm_domain::ports::PortError::new(prm_domain::ports::PortErrorKind::Failed, "task panicked")
}

fn port_panicked() -> Result<(), prm_domain::ports::PortError> {
    Err(port_panic_error())
}
