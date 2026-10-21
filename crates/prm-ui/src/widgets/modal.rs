//! A centered dialog on the shared dialog layout: a header with a close button, the body, and
//! right-aligned buttons, separated by full-width dividers. Esc closes it. It fits the window:
//! narrower than asked in a small window, and its body scrolls when it is taller than the room.

use eframe::egui::{self, RichText};

use crate::{
    theme::{UiTheme, adapter},
    widgets::{
        self,
        dialog::{self, DialogHead},
        fit,
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
    /// The width in a roomy window, in points; less when the window is narrower.
    pub width: f32,
}

/// The usual dialog width.
pub(crate) const WIDTH: f32 = 420.0;
/// The tallest a dialog's body gets before it scrolls, however tall the window (points).
const MAX_BODY: f32 = 520.0;
/// The least room a body keeps in a very short window (points).
const MIN_BODY: f32 = 80.0;

pub(crate) fn modal(
    ctx: &egui::Context,
    theme: &UiTheme,
    spec: &ModalSpec<'_>,
    body: impl FnOnce(&mut egui::Ui),
) -> ModalOutcome {
    let mut outcome = ModalOutcome::Open;
    let frame = adapter::frame_dialog(theme, theme.policy).inner_margin(egui::Margin::ZERO);
    let chrome = frame.total_margin().sum();
    let room = fit::room(ctx.content_rect().size()) - chrome;
    let id = egui::Id::unique(spec.id);
    let response = egui::Modal::new(id).frame(frame).show(ctx, |ui| {
        ui.set_width(spec.width.min(room.x));
        let top = ui.cursor().top();
        let head = DialogHead {
            title: spec.title,
            subtitle: spec.subtitle,
            display: false,
            close: true,
        };
        if dialog::header(ui, theme, &head) {
            outcome = ModalOutcome::Dismissed;
        }
        let bar = id.with("action_bar");
        let used = ui.cursor().top() - top + bar_height(ui, theme, spec, bar);
        let height = (room.y - used).clamp(MIN_BODY, MAX_BODY);
        egui::ScrollArea::vertical()
            .id_salt(id.with("body"))
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| dialog::body(ui, theme, body));
        if let Some(index) = action_bar(ui, theme, spec, bar) {
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

/// The action bar's height as last drawn (stored under `bar`), or a single row before then.
fn bar_height(ui: &egui::Ui, theme: &UiTheme, spec: &ModalSpec<'_>, bar: egui::Id) -> f32 {
    if spec.buttons.is_empty() {
        return 0.0;
    }
    let spacing = ui.spacing().item_spacing.y;
    let single = theme.control_height() + 2.0 * dialog::INSET + 2.0 * spacing + 1.0;
    ui.data(|d| d.get_temp(bar)).unwrap_or(single)
}

/// The divider and the buttons, which wrap onto more lines when they do not fit side by side;
/// remembers how tall it was under `bar`, so the body leaves room for it. Returns the clicked
/// button's index.
fn action_bar(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    spec: &ModalSpec<'_>,
    bar: egui::Id,
) -> Option<usize> {
    if spec.buttons.is_empty() {
        return None;
    }
    let top = ui.cursor().top();
    let right = egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true);
    let clicked = dialog::actions(ui, theme, |ui| {
        ui.with_layout(right, |ui| buttons(ui, theme, spec)).inner
    });
    let height = ui.cursor().top() - top;
    let before: Option<f32> = ui.data(|d| d.get_temp(bar));
    if before.is_none_or(|b| (b - height).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(bar, height));
        ui.ctx().request_repaint();
    }
    clicked
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
