//! The Save bar under the page: "You have unsaved changes" (or why the last save was not
//! written) with Discard and Save beside it, or above them when the page is too narrow for both.

use eframe::egui::{self, Stroke, UiBuilder, vec2};

use crate::{
    actions::UiAction, copy, frame::FrameCx, screens::settings::SettingsState, theme::UiTheme,
    widgets,
};

/// How the bar lays out, measured before the page so the page can end above it.
pub(super) struct SaveBar {
    /// The message's height when it sits above the buttons; `None` when it fits beside them.
    above: Option<f32>,
}

impl SaveBar {
    /// The bar's height with `inset` around its contents, its divider included.
    pub(super) fn height(&self, theme: &UiTheme, inset: f32) -> f32 {
        let message = self
            .above
            .map_or(0.0, |height| height + theme.tokens.metrics.space.sm);
        message + theme.control_height() + 2.0 * inset + 1.0
    }
}

fn message(st: &SettingsState) -> &str {
    st.save_error.as_deref().unwrap_or(copy::UNSAVED)
}

/// Measures the message and the buttons in a row `row` points wide: side by side when they fit,
/// else the message wraps above the buttons.
pub(super) fn layout(ui: &egui::Ui, theme: &UiTheme, st: &SettingsState, row: f32) -> SaveBar {
    let font = theme.fonts.small.clone();
    let color = theme.colors.text_secondary;
    let text = message(st).to_owned();
    let line = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), color)
        .size()
        .x;
    let mut buttons = widgets::button_width(ui, theme, copy::SAVE_CHANGES);
    if st.dirty() {
        buttons += ui.spacing().item_spacing.x + widgets::button_width(ui, theme, copy::DISCARD);
    }
    if line + theme.tokens.metrics.space.xl + buttons <= row {
        return SaveBar { above: None };
    }
    let wrapped = ui.painter().layout(text, font, color, row).size().y;
    SaveBar {
        above: Some(wrapped),
    }
}

/// Draws the bar to fill the `Ui`'s rect exactly: a divider, then the message and the buttons
/// `inset` from the edges.
pub(super) fn show(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &SettingsState,
    (bar, inset): (&SaveBar, f32),
) {
    let theme = cx.theme.clone();
    let rect = ui.max_rect();
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter().hline(rect.x_range(), rect.top(), stroke);
    let row = rect.shrink(inset);
    let right = egui::Layout::right_to_left(egui::Align::Center);
    let dirty = st.dirty();
    match bar.above {
        None => {
            let layout = egui::Layout::left_to_right(egui::Align::Center);
            ui.scope_builder(UiBuilder::new().max_rect(row).layout(layout), |ui| {
                message_label(ui, &theme, st);
                ui.with_layout(right, |ui| {
                    ui.add_enabled_ui(!st.saving, |ui| save_buttons(ui, cx, dirty));
                });
            });
        }
        Some(_) => {
            ui.scope_builder(UiBuilder::new().max_rect(row), |ui| {
                ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.sm;
                message_label(ui, &theme, st);
                let size = vec2(ui.available_width(), theme.control_height());
                ui.allocate_ui_with_layout(size, right, |ui| {
                    ui.add_enabled_ui(!st.saving, |ui| save_buttons(ui, cx, dirty));
                });
            });
        }
    }
}

fn message_label(ui: &mut egui::Ui, theme: &UiTheme, st: &SettingsState) {
    match &st.save_error {
        Some(error) => widgets::error_text(ui, theme, error),
        None => widgets::muted(ui, theme, copy::UNSAVED),
    };
}

/// Save (which also retries a save that could not be written), and Discard while dirty.
fn save_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, dirty: bool) {
    let theme = cx.theme.clone();
    if widgets::primary_button(ui, &theme, copy::SAVE_CHANGES).clicked() {
        cx.push(UiAction::SaveSettings);
    }
    if dirty && ui.button(copy::DISCARD).clicked() {
        cx.push(UiAction::DiscardSettings);
    }
}
