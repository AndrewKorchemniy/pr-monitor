//! Service events: activation, people search, sign-in, config and test-notification results.
//! Each result shows where the user is already looking; the rest only goes to the log.

use std::sync::Arc;

use eframe::egui;
use prm_domain::{OrgId, Person, PrKey, uri::AppUri};
use prm_service::{AppEvent, Command, UiError};
use zeroize::Zeroize as _;

use super::PrMonitorApp;
use crate::{
    copy,
    route::{ModalKind, Overlay},
};

impl PrMonitorApp {
    pub(super) fn drain_events(&mut self, ctx: &egui::Context) {
        while let Some(event) = self.link.try_event() {
            self.on_event(ctx, event);
        }
    }

    fn on_event(&mut self, ctx: &egui::Context, event: AppEvent) {
        match event {
            AppEvent::Activate(uri) => self.activate(ctx, uri),
            AppEvent::PeopleResults { .. } | AppEvent::PersonResolved { .. } => {
                if let Some(settings) = &mut self.settings {
                    settings.on_event(&event);
                }
                if let Some(first_run) = &mut self.first_run {
                    first_run.settings.on_event(&event);
                }
            }
            // why: the org's sign-in state and its banner already show the outcome.
            AppEvent::SignInFinished { org, result } => {
                if let Err(error) = result {
                    tracing::info!(org = %org, error = %error.title, "sign-in did not finish");
                }
            }
            AppEvent::PatResult { org, result } => self.on_pat_result(&org, result),
            AppEvent::ConfigApplied { saved, .. } => self.on_config_applied(saved),
            AppEvent::Changes(changes) => self.changes.record(&changes),
            AppEvent::NotificationTestResult(result) => self.on_test_result(result),
            AppEvent::Stopped => self.on_service_stopped(),
        }
    }

    /// A save from settings that could not be written keeps the Save bar up with the error;
    /// any later successful write clears it. Success needs no message: the bar goes away.
    fn on_config_applied(&mut self, saved: bool) {
        if !saved {
            tracing::warn!("settings were applied but not written to disk");
        }
        let Some(settings) = &mut self.settings else {
            return;
        };
        let asked = std::mem::take(&mut settings.confirm_pending);
        if saved {
            settings.save_error = None;
        } else if asked {
            let text = copy::changes_not_saved(copy::SAVE_FAILED);
            settings.save_error = Some(Arc::from(text));
        }
    }

    /// Shown beside the test button while settings is open; a failure is logged either way.
    fn on_test_result(&mut self, result: Result<(), UiError>) {
        if let Err(error) = &result {
            tracing::warn!(error = %error.title, "test notification failed");
        }
        if let Some(settings) = &mut self.settings {
            let message = |e: UiError| Arc::from(copy::test_failed(&e.title));
            settings.test_result = Some(result.map_err(message));
        }
    }

    fn on_pat_result(&mut self, org: &OrgId, result: Result<Person, UiError>) {
        let pat_open = matches!(&self.overlay, Overlay::Modal(ModalKind::Pat(open)) if open == org);
        match result {
            Ok(_) => {
                if pat_open {
                    self.overlay = Overlay::None;
                }
                self.pat_input.zeroize();
                tracing::info!(org = %org, "token saved");
            }
            Err(error) if pat_open => self.pat_error = Some(error.title),
            Err(error) => tracing::warn!(org = %org, error = %error.title, "token not saved"),
        }
    }

    /// `prmonitor://show?pr=…` brings the window forward on that PR; `refresh` refreshes.
    pub(super) fn activate(&mut self, ctx: &egui::Context, uri: AppUri) {
        match uri {
            AppUri::Show { key } => {
                self.window.show(ctx);
                if let Some(key) = key {
                    self.close_settings_if_clean();
                    self.reveal(&key);
                    self.dash.view.select_key(&key);
                }
            }
            AppUri::Refresh => self.link.send(Command::RefreshNow),
        }
    }

    /// Clears the filters hiding an activated PR and expands its group, so the toast the user
    /// clicked lands on a visible row.
    fn reveal(&mut self, key: &PrKey) {
        if self.dash.view.is_filtered_out(key) {
            self.clear_filters();
        }
        self.dash.view.expand_group_of(key);
    }

    fn close_settings_if_clean(&mut self) {
        if self.settings.as_ref().is_none_or(|s| !s.dirty()) {
            self.close_settings();
        }
    }
}
