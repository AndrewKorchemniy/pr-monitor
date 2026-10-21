//! The palette, the shortcut sheet and modal dialogs.

use eframe::egui::{self, RichText};
use prm_domain::{
    OrgId, Secret,
    app::DocLink,
    auth::{AzBroker, SignInMethod},
};
use prm_service::{AuthStateView, Command};

use super::{PrMonitorApp, frame_cx};
use crate::{
    actions::UiAction,
    commands::palette::{PaletteItem, PaletteSources},
    copy,
    route::{ModalKind, Overlay},
    screens::shortcuts::shortcuts,
    widgets::{
        self,
        modal::{ModalOutcome, ModalSpec, modal},
        palette_view::{PaletteOutcome, palette_view},
    },
};

impl PrMonitorApp {
    pub(super) fn draw_overlay(&mut self, ctx: &egui::Context) {
        match self.overlay.clone() {
            Overlay::None => {}
            Overlay::Palette => self.draw_palette(ctx),
            Overlay::Shortcuts => {
                let cx = frame_cx!(self, ctx);
                if shortcuts(ctx, &cx) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Modal(kind) => self.draw_modal(ctx, &kind),
        }
    }

    fn draw_palette(&mut self, ctx: &egui::Context) {
        let sign_in = self
            .state
            .orgs
            .iter()
            .filter(|o| o.enabled && !matches!(o.auth.state, AuthStateView::SignedIn))
            .map(|o| (o.id.clone(), o.label.to_string()))
            .collect();
        let themes = self
            .theme
            .registry()
            .list()
            .map(|(id, e)| (id.as_str().to_owned(), e.file.name.clone()))
            .collect();
        let rows = self.state.rows.clone();
        let sources = PaletteSources {
            sign_in,
            themes,
            rows: &rows,
        };
        let cx = frame_cx!(self, ctx);
        let outcome = palette_view(ctx, &cx, &mut self.palette, &sources);
        match outcome {
            PaletteOutcome::Open => {}
            PaletteOutcome::Close => self.overlay = Overlay::None,
            PaletteOutcome::Run(item) => {
                self.overlay = Overlay::None;
                self.run_palette_item(ctx, item);
            }
        }
    }

    fn run_palette_item(&mut self, ctx: &egui::Context, item: PaletteItem) {
        match item {
            PaletteItem::Command(command) => self.run_command(ctx, command),
            PaletteItem::SignIn(org) => {
                self.user_action(Some(org), prm_domain::auth::UserAction::SignInWithMicrosoft);
            }
            PaletteItem::Theme(id) => {
                let mut config = (*self.state.config).clone();
                config.ui.theme = id;
                self.link.send(Command::ApplyConfig(Box::new(config)));
            }
            PaletteItem::GoTo(key) => self.dash.view.select_key(&key),
        }
    }

    fn draw_modal(&mut self, ctx: &egui::Context, kind: &ModalKind) {
        let theme = self.theme.current(ctx).clone();
        let outcome = match kind {
            ModalKind::DiscardSettings => {
                let spec = ModalSpec {
                    id: "discard",
                    title: copy::DISCARD_TITLE,
                    subtitle: None,
                    buttons: &[copy::DISCARD, copy::KEEP_EDITING],
                    danger: true,
                    width: widgets::modal::WIDTH,
                };
                let outcome = modal(ctx, &theme, &spec, |ui| {
                    ui.label(copy::DISCARD_BODY);
                });
                if outcome == ModalOutcome::Button(0) {
                    self.close_settings();
                }
                outcome
            }
            ModalKind::RemoveOrg(index) => self.remove_org_modal(ctx, *index),
            ModalKind::Pat(org) => self.pat_modal(ctx, org),
            ModalKind::ContactAdmin(message) => {
                let spec = ModalSpec {
                    id: "admin",
                    title: copy::ADMIN_TITLE,
                    subtitle: None,
                    buttons: &[copy::COPY_MESSAGE, copy::CLOSE],
                    danger: false,
                    width: widgets::modal::WIDTH,
                };
                let outcome = modal(ctx, &theme, &spec, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(message).font(theme.fonts.mono.clone()))
                            .wrap(),
                    );
                });
                if outcome == ModalOutcome::Button(0) {
                    self.actions.push(UiAction::Copy(message.clone()));
                }
                outcome
            }
            ModalKind::AuthMenu => self.auth_menu(ctx),
        };
        if outcome != ModalOutcome::Open
            && matches!(&self.overlay, Overlay::Modal(open) if open == kind)
        {
            self.overlay = Overlay::None;
        }
    }

    fn remove_org_modal(&mut self, ctx: &egui::Context, index: usize) -> ModalOutcome {
        let theme = self.theme.current(ctx).clone();
        let label = self
            .settings
            .as_ref()
            .and_then(|s| s.draft.orgs.get(index))
            .map(|o| o.display_label().to_string())
            .unwrap_or_default();
        let title = copy::remove_org_title(&label);
        let spec = ModalSpec {
            id: "remove_org",
            title: &title,
            subtitle: None,
            buttons: &[copy::REMOVE, copy::CANCEL],
            danger: true,
            width: widgets::modal::WIDTH,
        };
        let outcome = modal(ctx, &theme, &spec, |ui| {
            ui.add(egui::Label::new(copy::REMOVE_ORG_BODY).wrap());
        });
        if outcome == ModalOutcome::Button(0)
            && let Some(settings) = &mut self.settings
            && index < settings.draft.orgs.len()
        {
            settings.draft.orgs.remove(index);
            settings.org = settings
                .org
                .min(settings.draft.orgs.len().saturating_sub(1));
        }
        outcome
    }

    fn pat_modal(&mut self, ctx: &egui::Context, org: &OrgId) -> ModalOutcome {
        let theme = self.theme.current(ctx).clone();
        let label = self
            .state
            .org(org)
            .map_or_else(|| org.as_str().to_owned(), |o| o.label.to_string());
        let title = copy::pat_title(&label);
        let spec = ModalSpec {
            id: "pat",
            title: &title,
            subtitle: None,
            buttons: &[copy::SAVE_TOKEN, copy::CANCEL],
            danger: false,
            width: widgets::modal::WIDTH,
        };
        let mut docs = false;
        let input = &mut self.pat_input;
        let error = self.pat_error.clone();
        let outcome = modal(ctx, &theme, &spec, |ui| {
            widgets::field_label(ui, &theme, copy::PAT_LABEL);
            ui.add(
                egui::TextEdit::singleline(input)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            widgets::muted(ui, &theme, copy::PAT_HELP);
            docs = ui.link(copy::HOW_TO_CREATE_TOKEN).clicked();
            if let Some(error) = &error {
                ui.label(
                    RichText::new(error.as_ref())
                        .color(theme.colors.tone(crate::theme::Tone::Danger).fg),
                );
            }
        });
        if docs && let Ok(url) = url::Url::parse(DocLink::CreatePat.url()) {
            self.link.send(Command::OpenUrl(url));
        }
        if outcome == ModalOutcome::Button(0) && !self.pat_input.trim().is_empty() {
            let pat = Secret::new(self.pat_input.trim());
            self.link.send(Command::SetPat {
                org: org.clone(),
                pat,
            });
            self.pat_error = None;
            // why: the dialog stays open until the service validates the token.
            return ModalOutcome::Open;
        }
        outcome
    }

    /// Sign in to every org that needs it, one row per org.
    fn auth_menu(&mut self, ctx: &egui::Context) -> ModalOutcome {
        let theme = self.theme.current(ctx).clone();
        let spec = ModalSpec {
            id: "auth_menu",
            title: copy::TRAY_SIGN_IN.trim_end_matches('…'),
            subtitle: None,
            buttons: &[copy::CLOSE],
            danger: false,
            width: widgets::modal::WIDTH,
        };
        let orgs: Vec<_> = self
            .state
            .orgs
            .iter()
            .filter(|o| o.enabled)
            .cloned()
            .collect();
        let mut chosen: Option<(OrgId, Option<SignInMethod>)> = None;
        let outcome = modal(ctx, &theme, &spec, |ui| {
            chosen = auth_rows(ui, &theme, &orgs);
        });
        match chosen {
            Some((org, Some(method))) => {
                self.link.send(Command::SignIn { org, method });
                ModalOutcome::Button(0)
            }
            Some((org, None)) => {
                self.open_pat(org);
                ModalOutcome::Open
            }
            None => outcome,
        }
    }
}

/// One row of sign-in buttons per org; the org and method clicked.
fn auth_rows(
    ui: &mut egui::Ui,
    theme: &crate::theme::UiTheme,
    orgs: &[prm_service::OrgStatus],
) -> Option<(OrgId, Option<SignInMethod>)> {
    let mut chosen = None;
    for org in orgs {
        let name = copy::identity(&org.label, org.auth.method);
        ui.label(RichText::new(name).font(theme.fonts.body_strong.clone()));
        let clicked = ui.horizontal_wrapped(|ui| sign_in_buttons(ui, theme)).inner;
        chosen = chosen.or_else(|| clicked.map(|method| (org.id.clone(), method)));
        ui.add_space(theme.tokens.metrics.space.md);
    }
    chosen
}

/// Some(Some(method)) to sign in, Some(None) for a token, None when nothing was clicked.
#[expect(
    clippy::option_option,
    reason = "the inner None means the token dialog"
)]
fn sign_in_buttons(
    ui: &mut egui::Ui,
    theme: &crate::theme::UiTheme,
) -> Option<Option<SignInMethod>> {
    let az = |broker| Some(Some(SignInMethod::AzLogin { broker }));
    if widgets::primary_button(ui, theme, copy::SIGN_IN_MICROSOFT).clicked() {
        return az(AzBroker::Default);
    }
    if ui.button(copy::SIGN_IN_BROWSER).clicked() {
        return az(AzBroker::Browser);
    }
    ui.button(copy::USE_PAT).clicked().then_some(None)
}
