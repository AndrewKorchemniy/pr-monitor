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
    widgets::{
        self, controls,
        fields::{self, Field},
        fit,
    },
};

/// The width of the Change sign-in menu.
const MENU_WIDTH: f32 = 260.0;
/// The side of the sign-in state's dot, in points.
const STATE_DOT: f32 = 8.0;

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
    headline(ui, cx, &status);
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

/// The state line with, once signed in, Change sign-in and Sign out at its right end, or on the
/// line below when they do not fit beside it.
fn headline(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    let theme = cx.theme.clone();
    if !matches!(status.auth.state, AuthStateView::SignedIn) {
        ui.horizontal_wrapped(|ui| state_line(ui, &theme, status));
        return;
    }
    let gap = ui.spacing().item_spacing.x;
    // why: the buttons' drawn width, kept from the last frame, so the last one ends on the
    // content's edge; the estimate only stands in until then.
    let id = ui.scope_id().with("sign_in_buttons");
    let buttons = widgets::recall(ui, id).unwrap_or_else(|| {
        widgets::button_width(ui, &theme, copy::CHANGE_SIGN_IN)
            + gap
            + widgets::button_width(ui, &theme, copy::SIGN_OUT)
    });
    if state_width(ui, &theme, status) + gap + buttons > ui.available_width() {
        ui.horizontal_wrapped(|ui| state_line(ui, &theme, status));
        ui.horizontal_wrapped(|ui| signed_in_buttons(ui, cx, status));
        return;
    }
    ui.horizontal(|ui| {
        state_line(ui, &theme, status);
        // why: laid out left to right after a gap, not right to left, so the buttons come in
        // the same reading and Tab order as on the line below.
        ui.add_space((ui.available_width() - buttons).max(0.0));
        let drawn = ui.horizontal(|ui| signed_in_buttons(ui, cx, status));
        widgets::remember(ui, id, drawn.response.rect.width());
    });
}

/// The state line's width on one line: the dot, the headline and "via Azure CLI".
fn state_width(ui: &egui::Ui, theme: &UiTheme, status: &OrgStatus) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    let measure = |text: String, font: egui::FontId| {
        let galley = ui
            .painter()
            .layout_no_wrap(text, font, theme.colors.text_primary);
        galley.size().x
    };
    let (text, _) = state_text(status);
    let mut width = STATE_DOT + gap + measure(text, theme.fonts.body_strong.clone());
    if let (AuthStateView::SignedIn, Some(method)) = (&status.auth.state, status.auth.method) {
        width += gap + measure(copy::via(method.label()), theme.fonts.small.clone());
    }
    width
}

/// The sign-in method, the tenant, and (only for the organization's app) its client ID and
/// tenant, as rows with right-aligned fields.
fn advanced(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    org: &mut prm_config::OrgProfile,
    status: &OrgStatus,
) {
    let theme = cx.theme.clone();
    widgets::field_label(ui, &theme, copy::SIGN_IN_METHOD);
    for (mode, label) in MODES {
        controls::radio(ui, &theme, &mut org.auth, mode, label);
    }
    controls::row_divider(ui, &theme);
    field_row(
        ui,
        &theme,
        (copy::TENANT_ID, Some(copy::TENANT_HELP)),
        &mut org.tenant,
    );
    if org.auth == AuthMode::Entra {
        entra_rows(ui, cx, org, status);
    }
    if let Some(expires) = status.pat_expires_on {
        controls::row_divider(ui, &theme);
        widgets::muted(ui, &theme, &format!("{} {expires}", copy::TOKEN_EXPIRES));
    }
}

/// A labelled row with a right-aligned text field.
fn field_row(ui: &mut egui::Ui, theme: &UiTheme, text: (&str, Option<&str>), value: &mut String) {
    controls::setting_row(ui, theme, text, |ui| {
        let id = ui.make_persistent_id(("auth_field", text.0));
        // why: the hint is also the field's accessible name.
        let field = Field::new(id, text.0).width(fit::CONTROL_WIDTH);
        fields::text_field(ui, theme, value, &field);
    });
}

/// The organization's app: its client ID and tenant, then the sign-in button.
fn entra_rows(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    org: &mut prm_config::OrgProfile,
    status: &OrgStatus,
) {
    let theme = cx.theme.clone();
    controls::row_divider(ui, &theme);
    field_row(
        ui,
        &theme,
        (copy::ENTRA_CLIENT_ID, Some(copy::ENTRA_HELP)),
        &mut org.entra.client_id,
    );
    controls::row_divider(ui, &theme);
    let tenant = (copy::ENTRA_TENANT, Some(copy::ENTRA_TENANT_HELP));
    field_row(ui, &theme, tenant, &mut org.entra.tenant);
    let ready = !org.entra.client_id.trim().is_empty();
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let button = ui.add_enabled(ready, egui::Button::new(copy::SIGN_IN_ENTRA));
        if button.clicked() {
            cx.push(UiAction::Send(Command::SignIn {
                org: status.id.clone(),
                method: SignInMethod::Entra,
            }));
        }
    });
}

/// The state dot and headline ("Signed in as Maya Patel", then "via Azure CLI" muted).
fn state_line(ui: &mut egui::Ui, theme: &UiTheme, status: &OrgStatus) {
    let (text, tone) = state_text(status);
    let icon = crate::theme::icons::Icon::Dot;
    crate::theme::icons::show(ui, icon, STATE_DOT, theme.colors.tone(tone).solid);
    ui.label(RichText::new(text).font(theme.fonts.body_strong.clone()));
    if let (AuthStateView::SignedIn, Some(method)) = (&status.auth.state, status.auth.method) {
        widgets::muted(ui, theme, &copy::via(method.label()));
    }
}

/// The headline ("Signed in as Maya Patel") and the tone of its dot.
fn state_text(status: &OrgStatus) -> (String, Tone) {
    match &status.auth.state {
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

/// Signed in: a menu with the other ways to sign in, then Sign out.
fn signed_in_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, status: &OrgStatus) {
    let response = ui.button(copy::CHANGE_SIGN_IN);
    let theme = cx.theme.clone();
    egui::Popup::menu(&response)
        .frame(controls::menu_frame(&theme))
        .gap(controls::MENU_GAP)
        .width(MENU_WIDTH)
        .show(|ui| {
            ui.set_width(MENU_WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            method_items(ui, cx, status);
        });
    if ui.button(copy::SIGN_OUT).clicked() {
        cx.push(UiAction::Send(Command::SignOut {
            org: status.id.clone(),
        }));
    }
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
