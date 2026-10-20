//! General: refresh interval, tray behaviour, launch at login, columns, times and proxy.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{OrgColumn, ProxyConfig},
};

use crate::{
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{
        card::section,
        controls::{self, setting_row, toggle_row},
    },
};

const ORG_COLUMN: [(OrgColumn, &str); 3] = [
    (OrgColumn::Auto, "Automatic"),
    (OrgColumn::Always, "Always"),
    (OrgColumn::Never, "Never"),
];

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, draft: &mut Config) {
    let theme = cx.theme.clone();
    section(ui, &theme, copy::SECTION_REFRESH, None, |ui| {
        let interval = &mut draft.refresh.interval_minutes;
        let label = copy::refresh_every(*interval);
        setting_row(ui, &theme, (label.trim(), Some(copy::REFRESH_HELP)), |ui| {
            controls::slider(ui, &theme, interval, limits::INTERVAL_MINUTES, label.trim());
        });
    });
    let prefs = &mut draft.ui;
    section(ui, &theme, copy::SECTION_WINDOW, None, |ui| {
        toggle_row(
            ui,
            &theme,
            &mut prefs.start_minimized,
            (copy::START_MINIMIZED, None),
        );
        ui.separator();
        toggle_row(
            ui,
            &theme,
            &mut prefs.close_to_tray,
            (copy::CLOSE_TO_TRAY, None),
        );
        ui.separator();
        toggle_row(
            ui,
            &theme,
            &mut prefs.minimize_to_tray,
            (copy::MINIMIZE_TO_TRAY, None),
        );
        ui.separator();
        let error = cx.state.autostart.error.as_deref();
        toggle_row(
            ui,
            &theme,
            &mut prefs.launch_at_login,
            (copy::LAUNCH_AT_LOGIN, error),
        );
    });
    section(ui, &theme, copy::SECTION_TABLE, None, |ui| {
        let help = Some(copy::ORG_COLUMN_HELP);
        setting_row(ui, &theme, (copy::ORG_COLUMN, help), |ui| {
            controls::segmented(ui, &theme, &mut prefs.show_org_column, &ORG_COLUMN);
        });
        ui.separator();
        toggle_row(
            ui,
            &theme,
            &mut prefs.relative_times,
            (copy::RELATIVE_TIMES, None),
        );
    });
    section(ui, &theme, copy::SECTION_NETWORK, None, |ui| {
        setting_row(ui, &theme, (copy::PROXY, Some(copy::RESTART_HELP)), |ui| {
            proxy(ui, &theme, &mut draft.network.proxy);
        });
    });
}

fn proxy(ui: &mut egui::Ui, theme: &UiTheme, proxy: &mut ProxyConfig) {
    let id = ui.make_persistent_id("proxy_text");
    let mut text: String = ui
        .data_mut(|d| d.get_temp(id))
        .unwrap_or_else(|| proxy.to_string());
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text(copy::PROXY_HINT)
            .font(theme.fonts.body.clone())
            .desired_width(260.0),
    );
    if response.changed() {
        if let Ok(parsed) = text.parse() {
            *proxy = parsed;
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    }
}
