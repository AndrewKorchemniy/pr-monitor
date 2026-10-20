//! A centered dialog with a title, a body and right-aligned buttons. Esc closes it.

use eframe::egui::{self, RichText};

use crate::{theme::UiTheme, widgets};

/// What the user did with the dialog this frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModalOutcome {
    Open,
    /// The primary button (index 0) or another button by index.
    Button(usize),
    Dismissed,
}

pub(crate) struct ModalSpec<'a> {
    pub id: &'a str,
    pub title: &'a str,
    /// Button labels; the first is the primary action.
    pub buttons: &'a [&'a str],
    pub danger: bool,
    pub width: f32,
}

/// The usual dialog width.
pub(crate) const WIDTH: f32 = 420.0;

pub(crate) fn modal(
    ctx: &egui::Context,
    theme: &UiTheme,
    spec: &ModalSpec<'_>,
    body: impl FnOnce(&mut egui::Ui),
) -> ModalOutcome {
    let mut outcome = ModalOutcome::Open;
    let response = egui::Modal::new(egui::Id::unique(spec.id))
        .frame(crate::theme::adapter::frame_popup(theme, theme.policy))
        .show(ctx, |ui| {
            ui.set_width(spec.width);
            widgets::heading(ui, theme, spec.title);
            ui.add_space(theme.tokens.metrics.space.md);
            body(ui);
            if spec.buttons.is_empty() {
                return;
            }
            ui.add_space(theme.tokens.metrics.space.lg);
            let right = egui::Layout::right_to_left(egui::Align::Center);
            if let Some(index) = ui.with_layout(right, |ui| buttons(ui, theme, spec)).inner {
                outcome = ModalOutcome::Button(index);
            }
        });
    if outcome == ModalOutcome::Open && response.should_close() {
        outcome = ModalOutcome::Dismissed;
    }
    outcome
}

/// The clicked button's index, if any.
fn buttons(ui: &mut egui::Ui, theme: &UiTheme, spec: &ModalSpec<'_>) -> Option<usize> {
    let mut clicked = None;
    for (index, label) in spec.buttons.iter().enumerate() {
        let response = if index == 0 {
            primary(ui, theme, label, spec.danger)
        } else {
            ui.button(*label)
        };
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

fn primary(ui: &mut egui::Ui, theme: &UiTheme, label: &str, danger: bool) -> egui::Response {
    if danger {
        let tone = theme.colors.tone(crate::theme::Tone::Danger);
        let text = RichText::new(label).color(theme.colors.text_on_accent);
        ui.add(egui::Button::new(text).fill(tone.solid))
    } else {
        widgets::primary_button(ui, theme, label)
    }
}
