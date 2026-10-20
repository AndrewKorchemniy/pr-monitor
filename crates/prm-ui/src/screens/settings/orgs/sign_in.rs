//! An org's connection: who is signed in and how, and the ways to sign in or out.

use eframe::egui::{self, RichText};
use prm_domain::auth::{AuthMode, AzBroker, SignInMethod};
use prm_service::{AuthStateView, Command, OrgStatus};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    route::ModalKind,
    screens::settings::SettingsState,
    theme::{Tone, UiTheme},
    widgets,
};

const MODES: [(AuthMode, &str); 4] = [
    (AuthMode::Auto, "Automatic (Azure CLI, then a saved token)"),
    (AuthMode::AzureCli, "Azure CLI only"),
    (AuthMode::Pat, "Personal access token only"),
    (AuthMode::Entra, "Your organization's app only"),
];

pub(super) fn section(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    saved: bool,
) {
    let theme = cx.theme.clone();
    let org = &mut st.draft.orgs[st.org];
    if !saved {
        widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        return;
    }
    let Some(status) = org.org_id().and_then(|id| cx.state.org(id)).cloned() else {
        widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        return;
    };
    state_line(ui, &theme, &status);
    ui.add_space(theme.tokens.metrics.space.md);
    buttons(ui, cx, &status);
    ui.add_space(theme.tokens.metrics.space.md);
    egui::CollapsingHeader::new(copy::ADVANCED)
        .id_salt("auth_advanced")
        .show(ui, |ui| {
            for (mode, label) in MODES {
                ui.radio_value(&mut org.auth, mode, label);
            }
            egui::Grid::new("entra").num_columns(2).show(ui, |ui| {
                widgets::field_label(ui, &theme, copy::TENANT_ID);
                ui.add(egui::TextEdit::singleline(&mut org.tenant).desired_width(320.0));
                ui.end_row();
                widgets::field_label(ui, &theme, copy::ENTRA_CLIENT_ID);
                ui.add(egui::TextEdit::singleline(&mut org.entra.client_id).desired_width(320.0));
                ui.end_row();
                widgets::field_label(ui, &theme, copy::ENTRA_TENANT);
                ui.add(egui::TextEdit::singleline(&mut org.entra.tenant).desired_width(320.0));
                ui.end_row();
            });
            widgets::muted(ui, &theme, copy::ENTRA_HELP);
            if !org.entra.client_id.trim().is_empty() && ui.button(copy::SIGN_IN_ENTRA).clicked() {
                cx.push(UiAction::Send(Command::SignIn {
                    org: status.id.clone(),
                    method: SignInMethod::Entra,
                }));
            }
            if let Some(expires) = status.pat_expires_on {
                widgets::muted(ui, &theme, &format!("{} {expires}", copy::TOKEN_EXPIRES));
            }
        });
}

fn state_line(ui: &mut egui::Ui, theme: &UiTheme, status: &OrgStatus) {
    let method = status.auth.method.map_or("", |m| m.label());
    let (text, tone, detail) = match &status.auth.state {
        AuthStateView::SignedIn => {
            let name = status
                .auth
                .me
                .as_ref()
                .map_or_else(|| status.label.clone(), prm_domain::Person::display_name);
            (copy::signed_in_as(&name, method), Tone::Success, None)
        }
        AuthStateView::Unknown | AuthStateView::Checking => {
            (copy::CHECKING_SIGN_IN.to_owned(), Tone::Info, None)
        }
        AuthStateView::SigningIn { .. } => (copy::WAITING_SIGN_IN.to_owned(), Tone::Info, None),
        AuthStateView::NeedsSignIn { error } => (
            copy::sign_in_needed(&error.title),
            Tone::Warning,
            error.detail.clone(),
        ),
        AuthStateView::Blocked { error } => (
            copy::blocked(&error.title),
            Tone::Danger,
            error.detail.clone(),
        ),
        AuthStateView::SignedOut | AuthStateView::Disabled => {
            (copy::SIGNED_OUT.to_owned(), Tone::Neutral, None)
        }
    };
    ui.horizontal(|ui| {
        crate::theme::icons::show(
            ui,
            crate::theme::icons::Icon::Dot,
            8.0,
            theme.colors.tone(tone).solid,
        );
        ui.label(RichText::new(text).font(theme.fonts.body_strong.clone()));
    });
    if let Some(detail) = detail {
        widgets::muted(ui, theme, &detail);
    }
}

fn buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    let theme = cx.theme.clone();
    let org = status.id.clone();
    let signing_in = matches!(status.auth.state, AuthStateView::SigningIn { .. });
    ui.horizontal_wrapped(|ui| {
        if signing_in {
            if ui.button(copy::CANCEL).clicked() {
                cx.push(UiAction::Send(Command::CancelSignIn { org: org.clone() }));
            }
            return;
        }
        let az_login = |broker| Command::SignIn {
            org: org.clone(),
            method: SignInMethod::AzLogin { broker },
        };
        if widgets::primary_button(ui, &theme, copy::SIGN_IN_MICROSOFT).clicked() {
            cx.push(UiAction::Send(az_login(AzBroker::Default)));
        }
        if ui.button(copy::SIGN_IN_BROWSER).clicked() {
            cx.push(UiAction::Send(az_login(AzBroker::Browser)));
        }
        if ui.button(copy::USE_PAT).clicked() {
            cx.push(UiAction::Modal(ModalKind::Pat(org.clone())));
        }
        if matches!(status.auth.state, AuthStateView::SignedIn)
            && ui.button(copy::SIGN_OUT).clicked()
        {
            cx.push(UiAction::Send(Command::SignOut { org: org.clone() }));
        }
    });
}
