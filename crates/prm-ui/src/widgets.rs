//! Reusable widgets. Each reads colors from the theme and reports intent through `UiAction`s.

pub(crate) mod banner;
pub(crate) mod card;
pub(crate) mod chip;
pub(crate) mod controls;
pub(crate) mod empty_state;
pub(crate) mod modal;
pub(crate) mod notice_stack;
pub(crate) mod palette_view;
pub(crate) mod peek;
pub(crate) mod people_picker;
pub(crate) mod pr_table;
pub(crate) mod scope_picker;
pub(crate) mod status_bar;
pub(crate) mod status_pill;
pub(crate) mod toolbar;
pub(crate) mod vote_tally;

use eframe::egui::{self, Color32, RichText};

use crate::theme::UiTheme;

/// A muted secondary line.
pub(crate) fn muted(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(theme.fonts.small.clone())
            .color(theme.colors.text_secondary),
    )
}

/// A section heading.
pub(crate) fn heading(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    ui.label(
        RichText::new(text)
            .font(theme.fonts.title.clone())
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

/// A key cap ("Ctrl+K").
pub(crate) fn kbd(ui: &mut egui::Ui, theme: &UiTheme, text: &str) -> egui::Response {
    egui::Frame::NONE
        .fill(theme.colors.control_bg)
        .stroke(egui::Stroke::new(1.0, theme.colors.border_subtle))
        .corner_radius(egui::CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(4, 1))
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
    ui.horizontal(|ui| {
        for chord in chords {
            kbd(ui, theme, chord);
        }
    });
}

/// A thin determinate progress line across `rect`'s bottom edge.
pub(crate) fn progress_line(ui: &egui::Ui, rect: egui::Rect, fraction: f32, color: Color32) {
    let line = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - 2.0),
        egui::pos2(
            rect.left() + rect.width() * fraction.clamp(0.0, 1.0),
            rect.bottom(),
        ),
    );
    ui.painter().rect_filled(line, 0.0, color);
}
