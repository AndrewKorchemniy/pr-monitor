//! Service events: notices, activation, people search, sign-in and config results.

use eframe::egui;
use prm_domain::{Timestamp, uri::AppUri};
use prm_service::{AppEvent, Command};

use super::PrMonitorApp;
use crate::{
    copy,
    route::{ModalKind, Overlay},
};

impl PrMonitorApp {
    pub(super) fn drain_events(&mut self, ctx: &egui::Context, now: Timestamp) {
        while let Some(event) = self.link.try_event() {
            self.on_event(ctx, event, now);
        }
    }

    fn on_event(&mut self, ctx: &egui::Context, event: AppEvent, now: Timestamp) {
        match event {
            AppEvent::Notice(notice) => self.notices.push(&notice, now),
            AppEvent::Activate(uri) => self.activate(ctx, uri),
            AppEvent::PeopleResults { .. } | AppEvent::PersonResolved { .. } => {
                if let Some(settings) = &mut self.settings {
                    settings.on_event(&event);
                }
                if let Some(first_run) = &mut self.first_run {
                    first_run.settings.on_event(&event);
                }
            }
            AppEvent::SignInFinished { result, .. } => {
                if let Err(error) = result
                    && !error.transient
                {
                    self.notices.info(&error.title, now);
                }
            }
            AppEvent::PatResult { org, result } => self.on_pat_result(&org, result, now),
            AppEvent::ConfigApplied { saved, .. } => self.on_config_applied(saved, now),
            AppEvent::Changes(changes) => self.changes.record(&changes),
            AppEvent::NotificationTestResult(result) => {
                let text = match result {
                    Ok(()) => copy::TEST_SENT.to_owned(),
                    Err(error) => copy::test_failed(&error.title),
                };
                self.notices.info(&text, now);
            }
            AppEvent::Stopped => self.on_service_stopped(),
        }
    }

    /// Confirms a settings save (the new config arrives with the state).
    fn on_config_applied(&mut self, saved: bool, now: Timestamp) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        if !std::mem::take(&mut settings.confirm_pending) {
            return;
        }
        let text = if saved {
            copy::CHANGES_SAVED.to_owned()
        } else {
            copy::changes_not_saved(copy::SAVE_FAILED)
        };
        self.notices.info(&text, now);
    }

    fn on_pat_result(
        &mut self,
        org: &prm_domain::OrgId,
        result: Result<prm_domain::Person, prm_service::UiError>,
        now: Timestamp,
    ) {
        let pat_open = matches!(&self.overlay, Overlay::Modal(ModalKind::Pat(open)) if open == org);
        match result {
            Ok(person) => {
                if pat_open {
                    self.overlay = Overlay::None;
                }
                self.pat_input.clear();
                self.notices
                    .info(&copy::token_saved(&person.display_name()), now);
            }
            Err(error) if pat_open => self.pat_error = Some(error.title),
            Err(error) => self.notices.info(&error.title, now),
        }
    }

    /// `prmonitor://show?pr=…` brings the window forward on that PR; `refresh` refreshes.
    pub(super) fn activate(&mut self, ctx: &egui::Context, uri: AppUri) {
        match uri {
            AppUri::Show { key } => {
                self.window.show(ctx);
                if let Some(key) = key {
                    self.close_settings_if_clean();
                    self.dash.view.select_key(&key);
                }
            }
            AppUri::Refresh => self.link.send(Command::RefreshNow),
        }
    }

    fn close_settings_if_clean(&mut self) {
        if self.settings.as_ref().is_none_or(|s| !s.dirty()) {
            self.close_settings();
        }
    }
}
