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
    /// A muted line under the title, such as the organization being edited.
    pub subtitle: Option<&'a str>,
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
        .frame(crate::theme::adapter::frame_dialog(theme, theme.policy))
        .show(ctx, |ui| {
            ui.set_width(spec.width);
            if header(ui, theme, spec) {
                outcome = ModalOutcome::Dismissed;
            }
            ui.add_space(theme.tokens.metrics.space.lg);
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

/// The title (and subtitle) with a close button on the right; true when closed.
fn header(ui: &mut egui::Ui, theme: &UiTheme, spec: &ModalSpec<'_>) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
            widgets::heading(ui, theme, spec.title);
            if let Some(subtitle) = spec.subtitle {
                widgets::muted(ui, theme, subtitle);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            closed = widgets::controls::close_button(ui, theme, crate::copy::CLOSE).clicked();
        });
    });
    closed
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
