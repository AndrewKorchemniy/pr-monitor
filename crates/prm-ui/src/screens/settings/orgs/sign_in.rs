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
    widgets::{self, controls},
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
    let status = org.org_id().and_then(|id| cx.state.org(id)).cloned();
    let Some(status) = status.filter(|_| saved) else {
        widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        return;
    };
    ui.horizontal(|ui| {
        state_line(ui, &theme, &status);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if matches!(status.auth.state, AuthStateView::SignedIn) {
                signed_in_buttons(ui, cx, &status);
            }
        });
    });
    if let Some(detail) = detail(&status) {
        widgets::muted(ui, &theme, &detail);
    }
    if !matches!(status.auth.state, AuthStateView::SignedIn) {
        ui.add_space(theme.tokens.metrics.space.sm);
        sign_in_buttons(ui, cx, &status);
    }
    ui.add_space(theme.tokens.metrics.space.xs);
    let open_id = ui.make_persistent_id("auth_advanced");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    controls::disclosure(ui, &theme, copy::ADVANCED, &mut open);
    ui.data_mut(|d| d.insert_temp(open_id, open));
    if open {
        advanced(ui, cx, org, &status);
    }
}

fn advanced(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    org: &mut prm_config::OrgProfile,
    status: &OrgStatus,
) {
    let theme = cx.theme.clone();
    for (mode, label) in MODES {
        controls::radio(ui, &theme, &mut org.auth, mode, label);
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
}

/// The state dot and headline ("Signed in as Maya Patel", then "via Azure CLI" muted).
fn state_line(ui: &mut egui::Ui, theme: &UiTheme, status: &OrgStatus) {
    let (text, tone) = match &status.auth.state {
        AuthStateView::SignedIn => {
            let name = status
                .auth
                .me
                .as_ref()
                .map_or_else(|| status.label.clone(), prm_domain::Person::display_name);
            (copy::signed_in_as(&name), Tone::Success)
        }
        AuthStateView::Unknown | AuthStateView::Checking => {
            (copy::CHECKING_SIGN_IN.to_owned(), Tone::Info)
        }
        AuthStateView::SigningIn { .. } => (copy::WAITING_SIGN_IN.to_owned(), Tone::Info),
        AuthStateView::NeedsSignIn { error } => (copy::sign_in_needed(&error.title), Tone::Warning),
        AuthStateView::Blocked { error } => (copy::blocked(&error.title), Tone::Danger),
        AuthStateView::SignedOut | AuthStateView::Disabled => {
            (copy::SIGNED_OUT.to_owned(), Tone::Neutral)
        }
    };
    let icon = crate::theme::icons::Icon::Dot;
    crate::theme::icons::show(ui, icon, 8.0, theme.colors.tone(tone).solid);
    ui.label(RichText::new(text).font(theme.fonts.body_strong.clone()));
    if let (AuthStateView::SignedIn, Some(method)) = (&status.auth.state, status.auth.method) {
        widgets::muted(ui, theme, &copy::via(method.label()));
    }
}

fn detail(status: &OrgStatus) -> Option<String> {
    match &status.auth.state {
        AuthStateView::NeedsSignIn { error } | AuthStateView::Blocked { error } => {
            error.detail.as_ref().map(ToString::to_string)
        }
        AuthStateView::SignedIn
        | AuthStateView::Unknown
        | AuthStateView::Checking
        | AuthStateView::SigningIn { .. }
        | AuthStateView::SignedOut
        | AuthStateView::Disabled => None,
    }
}

/// Signed in: Sign out, and a menu with the other ways to sign in.
fn signed_in_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    if ui.button(copy::SIGN_OUT).clicked() {
        cx.push(UiAction::Send(Command::SignOut {
            org: status.id.clone(),
        }));
    }
    let response = ui.button(copy::CHANGE_SIGN_IN);
    let theme = cx.theme.clone();
    egui::Popup::menu(&response)
        .frame(controls::menu_frame(&theme))
        .show(|ui| {
            ui.set_min_width(240.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            method_items(ui, cx, status);
        });
}

/// Not signed in: Microsoft sign-in as the primary action, the alternatives beside it.
fn sign_in_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    let theme = cx.theme.clone();
    ui.horizontal_wrapped(|ui| {
        if matches!(status.auth.state, AuthStateView::SigningIn { .. }) {
            if ui.button(copy::CANCEL).clicked() {
                cx.push(UiAction::Send(Command::CancelSignIn {
                    org: status.id.clone(),
                }));
            }
            return;
        }
        if widgets::primary_button(ui, &theme, copy::SIGN_IN_MICROSOFT).clicked() {
            cx.push(UiAction::Send(az_login(status, AzBroker::Default)));
        }
        if ui.button(copy::SIGN_IN_BROWSER).clicked() {
            cx.push(UiAction::Send(az_login(status, AzBroker::Browser)));
        }
        if ui.button(copy::USE_PAT).clicked() {
            cx.push(UiAction::Modal(ModalKind::Pat(status.id.clone())));
        }
    });
}

fn method_items(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    let theme = cx.theme.clone();
    if controls::menu_item(ui, &theme, copy::SIGN_IN_MICROSOFT, false).clicked() {
        cx.push(UiAction::Send(az_login(status, AzBroker::Default)));
    }
    if controls::menu_item(ui, &theme, copy::SIGN_IN_BROWSER, false).clicked() {
        cx.push(UiAction::Send(az_login(status, AzBroker::Browser)));
    }
    if controls::menu_item(ui, &theme, copy::USE_PAT, false).clicked() {
        cx.push(UiAction::Modal(ModalKind::Pat(status.id.clone())));
    }
}

fn az_login(status: &OrgStatus, broker: AzBroker) -> Command {
    Command::SignIn {
        org: status.id.clone(),
        method: SignInMethod::AzLogin { broker },
    }
}
