//! Drawing the current screen and running the actions widgets queued.

use std::sync::Arc;

use eframe::egui::{self, Key, Modifiers};
use prm_domain::{
    OrgId,
    auth::{AzBroker, SettingsSection, SignInMethod, UserAction},
};
use prm_service::Command;

use super::{PrMonitorApp, frame_cx};
use crate::{
    actions::UiAction,
    commands::keymap::ActiveScope,
    copy,
    route::{EscapeInputs, EscapeTarget, ModalKind, Overlay, Screen, escape_target},
    screens::{
        dashboard::dashboard,
        first_run::first_run,
        settings::{SettingsEnv, SettingsState, settings_modal},
    },
    widgets::{notice_stack::notice_stack, toolbar::filter_id},
};

impl PrMonitorApp {
    pub(super) fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let focused = self.window.focused();
        if let Some(fr) = &mut self.first_run {
            let mut cx = frame_cx!(self, &ctx);
            first_run(ui, &mut cx, fr);
        } else {
            let mut cx = frame_cx!(self, &ctx);
            dashboard(ui, &mut cx, &mut self.dash, focused);
        }
        if self.first_run.is_none()
            && self.screen == Screen::Settings
            && let Some(settings) = &mut self.settings
        {
            let renderer_line = renderer_line(self.render, self.adapter.as_ref());
            let env = SettingsEnv {
                themes: self
                    .theme
                    .registry()
                    .list()
                    .map(|(id, entry)| (id.as_str().to_owned(), entry.file.name.clone()))
                    .collect(),
                themes_dir: self.theme.themes_dir().to_path_buf(),
                theme_issues: self
                    .theme
                    .issues
                    .iter()
                    .map(|i| format!("{}: {}", i.theme.as_str(), i.message))
                    .collect(),
                build: self.build,
                licenses: self.licenses,
                renderer_line: &renderer_line,
            };
            let mut cx = frame_cx!(self, &ctx);
            settings_modal(&ctx, &mut cx, settings, &env);
        }
        self.draw_overlay(&ctx);
        // why: notices sit under the toolbar, clear of the search field and buttons.
        let inset = self.theme.current(&ctx).tokens.metrics.row.toolbar_height;
        let mut cx = frame_cx!(self, &ctx);
        notice_stack(&ctx, &mut cx, &self.notices, inset);
    }

    /// Keyboard shortcuts for the active scope, then Esc.
    pub(super) fn dispatch_keys(&mut self, ctx: &egui::Context) {
        let text_input = ctx.egui_wants_keyboard_input();
        let scope = if self.overlay != Overlay::None {
            ActiveScope::Overlay
        } else if text_input {
            ActiveScope::TextInput
        } else if self.screen == Screen::Settings || self.first_run.is_some() {
            ActiveScope::Settings
        } else {
            ActiveScope::Table
        };
        for command in self.keymap.dispatch(ctx, scope) {
            self.run_command(ctx, command);
        }
        if self.overlay == Overlay::None
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.escape(ctx, text_input);
        }
    }

    fn escape(&mut self, ctx: &egui::Context, text_input: bool) {
        let filter_focused = ctx.memory(|m| m.has_focus(filter_id()));
        let input = EscapeInputs {
            // why: a settings dialog (a picker) is a modal egui closes on Esc by itself.
            popup_open: egui::Popup::is_any_open(ctx)
                || self.settings.as_ref().is_some_and(|s| s.editing.is_some()),
            overlay: self.overlay.clone(),
            filter_focused: filter_focused || (text_input && self.dash.filter_focused),
            filter_empty: self.dash.filter.is_empty(),
            peek_open: self.dash.peek.open,
            screen: self.screen,
            filters_active: self.dash.view.filters_active(),
            has_tray: self.tray.is_some(),
        };
        match escape_target(&input) {
            EscapeTarget::Popup | EscapeTarget::CloseOverlay | EscapeTarget::Nothing => {}
            EscapeTarget::ClearFilterText => self.set_filter_text(String::new()),
            EscapeTarget::BlurFilter => ctx.memory_mut(|m| m.surrender_focus(filter_id())),
            EscapeTarget::ClosePeek => self.dash.peek.open = false,
            EscapeTarget::LeaveSettings => self.leave_settings(),
            EscapeTarget::ClearFilters => self.clear_filters(),
            EscapeTarget::HideToTray => self.hide(ctx),
        }
    }

    pub(super) fn set_filter_text(&mut self, text: String) {
        self.dash.view.set_text(&text);
        self.dash.filter = text;
    }

    pub(super) fn clear_filters(&mut self) {
        self.dash.filter.clear();
        self.dash.view.clear_filters();
    }

    pub(super) fn run_actions(&mut self, ctx: &egui::Context) {
        // why: an action may queue more (a palette item runs a command that copies a link).
        for _ in 0..4 {
            let actions = self.actions.take();
            if actions.is_empty() {
                break;
            }
            for action in actions {
                self.apply(ctx, action);
            }
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: UiAction) {
        let now = self.clock.now();
        match action {
            UiAction::Run(command) => self.run_command(ctx, command),
            UiAction::Send(command) => self.link.send(command),
            UiAction::User { org, action } => self.user_action(org, action),
            UiAction::OpenSettings(section) => self.open_settings(section),
            UiAction::Modal(kind) => self.overlay = Overlay::Modal(kind),
            UiAction::Select(key) => self.dash.view.select_key(&key),
            UiAction::Open(key) => self.open_pr(key),
            UiAction::SetStatus(status) => self.dash.view.set_status(status),
            UiAction::ClearFilters => self.clear_filters(),
            UiAction::SetFilterText(text) => self.set_filter_text(text),
            UiAction::Copy { text, notice } => {
                ctx.copy_text(text);
                self.notices.info(notice, now);
            }
            UiAction::DismissBanner(id) => {
                self.dash.dismissed.insert(id);
            }
            UiAction::DismissNotice(index) => self.notices.dismiss(index),
            UiAction::Sort(key) => self.dash.view.sort_by(key),
            UiAction::ToggleGroup(span) => self.dash.view.toggle_group(span),
            UiAction::SaveSettings => self.save_settings(),
            UiAction::FinishFirstRun => self.first_run = None,
            UiAction::LeaveSettings => self.leave_settings(),
            UiAction::DiscardSettings => self.discard_settings(),
            UiAction::ReloadThemes => self.theme.reload(),
            UiAction::PreviewTheme(theme) => self.theme.set_preview(theme),
        }
    }

    pub(super) fn open_pr(&mut self, key: prm_domain::PrKey) {
        self.changes.forget(&key);
        self.link.send(Command::OpenPr(key));
    }

    /// An org to act on: the given one, else the first enabled org that needs sign-in.
    fn target_org(&self, org: Option<OrgId>) -> Option<OrgId> {
        org.or_else(|| {
            self.state
                .orgs
                .iter()
                .find(|o| {
                    o.enabled && !matches!(o.auth.state, prm_service::AuthStateView::SignedIn)
                })
                .or_else(|| self.state.orgs.first())
                .map(|o| o.id.clone())
        })
    }

    pub(super) fn user_action(&mut self, org: Option<OrgId>, action: UserAction) {
        let target = self.target_org(org.clone());
        let sign_in = |method| target.clone().map(|org| Command::SignIn { org, method });
        match action {
            UserAction::SignInWithMicrosoft => {
                if let Some(command) = sign_in(SignInMethod::AzLogin {
                    broker: AzBroker::Default,
                }) {
                    self.link.send(command);
                }
            }
            UserAction::SignInEntra => {
                if let Some(command) = sign_in(SignInMethod::Entra) {
                    self.link.send(command);
                }
            }
            UserAction::EnterPat => {
                if let Some(org) = target {
                    self.open_pat(org);
                }
            }
            UserAction::ChooseSignIn => self.overlay = Overlay::Modal(ModalKind::AuthMenu),
            UserAction::Retry => self
                .link
                .send(org.map_or(Command::RefreshNow, Command::RefreshOrg)),
            UserAction::ContactAdmin { message } => {
                self.overlay = Overlay::Modal(ModalKind::ContactAdmin(message.to_string()));
            }
            UserAction::OpenDocs(link) => {
                if let Ok(url) = url::Url::parse(link.url()) {
                    self.link.send(Command::OpenUrl(url));
                }
            }
            UserAction::OpenSettings(section) => self.open_settings(section),
        }
    }

    pub(super) fn open_pat(&mut self, org: OrgId) {
        self.pat_input.clear();
        self.pat_error = None;
        self.overlay = Overlay::Modal(ModalKind::Pat(org));
    }

    pub(super) fn open_settings(&mut self, section: SettingsSection) {
        if self.settings.is_none() {
            self.settings = Some(SettingsState::new(Arc::clone(&self.state.config), section));
        } else if let Some(settings) = &mut self.settings {
            settings.page = crate::screens::settings::Page::from_section(section);
        }
        self.screen = Screen::Settings;
        self.overlay = Overlay::None;
    }

    fn save_settings(&mut self) {
        if let Some(settings) = &mut self.settings
            && settings.dirty()
        {
            settings.saving = true;
            settings.confirm_pending = true;
            self.link
                .send(Command::ApplyConfig(Box::new(settings.draft.clone())));
            self.theme.set_preview(None);
        }
    }

    pub(super) fn leave_settings(&mut self) {
        match &self.settings {
            Some(settings) if settings.dirty() => {
                self.overlay = Overlay::Modal(ModalKind::DiscardSettings);
            }
            Some(_) | None => self.close_settings(),
        }
    }

    pub(super) fn discard_settings(&mut self) {
        if let Some(settings) = &mut self.settings {
            settings.draft = (*settings.base).clone();
        }
        self.theme.set_preview(None);
    }

    pub(super) fn close_settings(&mut self) {
        self.settings = None;
        self.theme.set_preview(None);
        self.screen = Screen::Dashboard;
        self.overlay = Overlay::None;
    }
}

/// "Using Direct3D 12 on NVIDIA RTX 4070".
fn renderer_line(
    render: crate::render::RenderEnv,
    adapter: Option<&crate::render::AdapterFacts>,
) -> String {
    let name = adapter.map_or("this computer", |a| a.name.as_str());
    copy::using_renderer(
        render.renderer.api(),
        name,
        adapter.is_some_and(|a| a.software),
    )
}
