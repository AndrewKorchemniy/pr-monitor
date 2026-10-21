//! A centered dialog on the shared dialog layout: a header with a close button, the body, and
//! right-aligned buttons, separated by full-width dividers. Esc closes it.

use eframe::egui::{self, RichText};

use crate::{
    theme::{UiTheme, adapter},
    widgets::{
        self,
        dialog::{self, DialogHead},
    },
};

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
    let frame = adapter::frame_dialog(theme, theme.policy).inner_margin(egui::Margin::ZERO);
    let response = egui::Modal::new(egui::Id::unique(spec.id))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(spec.width);
            let head = DialogHead {
                title: spec.title,
                subtitle: spec.subtitle,
                display: false,
                close: true,
            };
            if dialog::header(ui, theme, &head) {
                outcome = ModalOutcome::Dismissed;
            }
            dialog::body(ui, theme, body);
            if spec.buttons.is_empty() {
                return;
            }
            let right = egui::Layout::right_to_left(egui::Align::Center);
            let clicked = dialog::actions(ui, theme, |ui| {
                ui.with_layout(right, |ui| buttons(ui, theme, spec)).inner
            });
            if let Some(index) = clicked {
                outcome = ModalOutcome::Button(index);
            }
        });
    // why: a click beside a dialog (or on a browser window over it) must not throw away what
    // was typed or chosen; Esc and the close button dismiss it.
    let escaped = response.should_close() && !response.backdrop_response.clicked();
    if outcome == ModalOutcome::Open && escaped {
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
