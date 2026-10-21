//! General: refresh interval, tray behaviour, launch at login, columns, times and proxy.

use eframe::egui;
use prm_config::{
    Config, limits,
    schema::{OrgColumn, ProxyConfig},
};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    theme::UiTheme,
    view::columns::{self, Choices},
    widgets::{
        self,
        card::section,
        controls::{self, setting_row, toggle_row},
        fields::{self, Field},
        pr_table::column_choice,
        slider::{SLIDER_WIDTH, SliderSpec, slider_field},
    },
};

const ORG_COLUMN: [(OrgColumn, &str); 3] = [
    (OrgColumn::Auto, "Automatic"),
    (OrgColumn::Always, "Always"),
    (OrgColumn::Never, "Never"),
];

/// The most boxes on a line of the column checklist.
const COLUMN_GRID: usize = 3;
/// Each box and its label in the column checklist (points).
const COLUMN_CHECK_WIDTH: f32 = 160.0;

/// `columns` are the table's live column choices (see `SettingsEnv::columns`).
pub(crate) fn page(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    draft: &mut Config,
    columns: Choices<'_>,
) {
    let theme = cx.theme.clone();
    section(ui, &theme, copy::SECTION_REFRESH, None, |ui| {
        refresh_row(ui, &theme, &mut draft.refresh.interval_minutes);
    });
    let prefs = &mut draft.ui;
    section(ui, &theme, copy::SECTION_WINDOW, None, |ui| {
        let error = cx.state.autostart.error.as_deref();
        window_rows(ui, &theme, prefs, error);
    });
    section(ui, &theme, copy::SECTION_TABLE, None, |ui| {
        let help = Some(copy::ORG_COLUMN_HELP);
        setting_row(ui, &theme, (copy::ORG_COLUMN, help), |ui| {
            controls::segmented(ui, &theme, &mut prefs.show_org_column, &ORG_COLUMN);
        });
        controls::row_divider(ui, &theme);
        toggle_row(
            ui,
            &theme,
            &mut prefs.relative_times,
            (copy::RELATIVE_TIMES, None),
        );
        controls::row_divider(ui, &theme);
        column_checklist(ui, cx, columns);
    });
    section(ui, &theme, copy::SECTION_NETWORK, None, |ui| {
        setting_row(ui, &theme, (copy::PROXY, Some(copy::RESTART_HELP)), |ui| {
            proxy(ui, &theme, &mut draft.network.proxy);
        });
    });
    section(ui, &theme, copy::SECTION_PRIVACY, None, |ui| {
        widgets::muted(ui, &theme, copy::PRIVACY);
    });
}

/// The same choices as right-clicking a column header, in table order. Status and Title are
/// listed but locked on. These apply right away: they are table layout, not saved settings.
fn column_checklist(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, choices: Choices<'_>) {
    let theme = cx.theme.clone();
    let m = &theme.tokens.metrics;
    ui.add_space(m.space.sm);
    ui.label(
        egui::RichText::new(copy::COLUMNS)
            .font(theme.fonts.body.clone())
            .color(theme.colors.text_primary),
    );
    widgets::muted(ui, &theme, copy::COLUMNS_HELP);
    ui.add_space(m.space.sm);
    let gap = m.space.xl;
    let per_line = checks_per_line(ui.available_width(), gap);
    egui::Grid::new("settings_columns")
        .num_columns(per_line)
        .min_col_width(COLUMN_CHECK_WIDTH)
        .spacing(egui::vec2(gap, m.space.xs))
        .show(ui, |ui| {
            for (index, column) in columns::listed().enumerate() {
                let check = choices.check(&column);
                let response = column_choice(ui, &theme, &column, check, |ui, state, label| {
                    controls::check(ui, &theme, state, label)
                });
                if response.clicked() {
                    cx.push(UiAction::ToggleColumn(column.id));
                }
                if (index + 1) % per_line == 0 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(m.space.sm);
}

/// How many column checks fit side by side in `width` points with `gap` between them: up to
/// `COLUMN_GRID`, and at least one.
fn checks_per_line(width: f32, gap: f32) -> usize {
    (1..=COLUMN_GRID)
        .rev()
        .find(|&n| n as f32 * (COLUMN_CHECK_WIDTH + gap) - gap <= width)
        .unwrap_or(1)
}

fn refresh_row(ui: &mut egui::Ui, theme: &UiTheme, interval: &mut u32) {
    let text = (copy::REFRESH_INTERVAL, Some(copy::REFRESH_HELP));
    setting_row(ui, theme, text, |ui| {
        let spec = SliderSpec {
            id: egui::Id::unique("refresh_interval"),
            name: copy::REFRESH_INTERVAL,
            scale: 1.0,
            unit: copy::UNIT_MINUTES,
            width: SLIDER_WIDTH,
        };
        slider_field(ui, theme, interval, limits::INTERVAL_MINUTES, &spec);
    });
}

/// Tray and startup switches, one row each with dividers between.
fn window_rows(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    prefs: &mut prm_config::UiPrefs,
    autostart_error: Option<&str>,
) {
    let rows = [
        (&mut prefs.start_minimized, copy::START_MINIMIZED, None),
        (&mut prefs.close_to_tray, copy::CLOSE_TO_TRAY, None),
        (&mut prefs.minimize_to_tray, copy::MINIMIZE_TO_TRAY, None),
        (
            &mut prefs.launch_at_login,
            copy::LAUNCH_AT_LOGIN,
            autostart_error,
        ),
    ];
    for (index, (value, label, help)) in rows.into_iter().enumerate() {
        if index > 0 {
            controls::row_divider(ui, theme);
        }
        toggle_row(ui, theme, value, (label, help));
    }
}

fn proxy(ui: &mut egui::Ui, theme: &UiTheme, proxy: &mut ProxyConfig) {
    let id = ui.make_persistent_id("proxy_text");
    let mut text: String = ui
        .data_mut(|d| d.get_temp(id))
        .unwrap_or_else(|| proxy.to_string());
    let field = Field::new(egui::Id::unique("proxy_field"), copy::PROXY_HINT).width(280.0);
    let response = fields::text_field(ui, theme, &mut text, &field);
    if response.changed() {
        if let Ok(parsed) = text.parse() {
            *proxy = parsed;
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    }
}

#[cfg(test)]
mod tests;
