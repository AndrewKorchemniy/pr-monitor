//! Reusable widgets. Each reads colors from the theme and reports intent through `UiAction`s.

pub(crate) mod banner;
pub(crate) mod card;
pub(crate) mod controls;
pub(crate) mod dialog;
pub(crate) mod empty_state;
pub(crate) mod fields;
pub(crate) mod fit;
pub(crate) mod modal;
pub(crate) mod palette_view;
pub(crate) mod peek;
pub(crate) mod people_picker;
pub(crate) mod pr_cards;
pub(crate) mod pr_table;
pub(crate) mod progress;
pub(crate) mod scope_picker;
pub(crate) mod slider;
pub(crate) mod status_bar;
pub(crate) mod status_pill;
pub(crate) mod toolbar;
pub(crate) mod vote_tally;

use eframe::egui::{self, RichText};

use crate::theme::UiTheme;

/// A muted secondary line.
pub(crate) fn muted(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(theme.fonts.small.clone())
            .color(theme.colors.text_secondary),
    )
}

/// A page title: the title face at a larger size.
pub(crate) fn page_title_font(theme: &UiTheme) -> egui::FontId {
    egui::FontId::new(20.0, theme.fonts.title.family.clone())
}

/// A page title.
pub(crate) fn page_title(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(page_title_font(theme))
            .color(theme.colors.text_primary),
    )
}

/// A validation or failure message in the danger tone.
pub(crate) fn error_text(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(theme.fonts.small.clone())
            .color(theme.colors.tone(crate::theme::Tone::Danger).fg),
    )
}

/// A label for a form field.
pub(crate) fn field_label(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(theme.fonts.label.clone())
            .color(theme.colors.text_secondary),
    )
}

/// A primary (accent) button.
pub(crate) fn primary_button(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    let label = RichText::new(text)
        .font(theme.fonts.button.clone())
        .color(theme.colors.text_on_accent);
    ui.add(egui::Button::new(label).fill(theme.colors.accent_base))
}

/// A plain button whose label is in the danger tone, for a destructive action that asks to be
/// confirmed.
pub(crate) fn danger_button(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    let label = RichText::new(text)
        .font(theme.fonts.button.clone())
        .color(theme.colors.tone(crate::theme::Tone::Danger).fg);
    ui.add(egui::Button::new(label))
}

/// The width a button labelled `text` takes (`primary_button` or `ui.button`), for laying out
/// a row of buttons before drawing it.
pub(crate) fn button_width(ui: &egui::Ui, theme: &UiTheme, text: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme.fonts.button.clone(),
        theme.colors.text_primary,
    );
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// A size the layout measured under `id` on an earlier pass, if any (`remember`).
pub(crate) fn recall(ui: &egui::Ui, id: egui::Id) -> Option<f32> {
    ui.data(|d| d.get_temp(id))
}

/// Keeps `measured` under `id` for `recall`, for layouts that need a size before they can draw
/// what has it. When it moved by more than half a point, the pass is laid out again, so a
/// layout that used the old size is never shown.
pub(crate) fn remember(ui: &egui::Ui, id: egui::Id, measured: f32) {
    if recall(ui, id).is_none_or(|kept| (kept - measured).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, measured));
        ui.ctx().request_discard("a remembered size changed");
    }
}

/// A key cap ("Ctrl+K").
pub(crate) fn kbd(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    egui::Frame::NONE
        .fill(theme.colors.surface_sunken)
        .stroke(egui::Stroke::new(1.0, theme.colors.border_default))
        .corner_radius(egui::CornerRadius::same(theme.tokens.metrics.radius.sm))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(theme.fonts.mono.clone())
                    .color(theme.colors.text_secondary),
            )
        })
        .inner
}

/// Several key caps in a row ("F5  Ctrl+R").
pub(crate) fn kbd_row(ui: &mut egui::Ui, theme: &UiTheme, chords: &[String]) {
    ui.scope(|ui| {
        // why: a horizontal row is at least the interact height; caps hug their label.
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal(|ui| {
            for chord in chords {
                kbd(ui, theme, chord);
            }
        });
    });
}
