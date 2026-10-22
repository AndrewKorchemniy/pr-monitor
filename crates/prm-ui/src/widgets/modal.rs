//! A centered dialog on the shared dialog layout: a header with a close button, the body, and
//! right-aligned buttons, separated by full-width dividers. Esc closes it. It fits the window:
//! narrower than asked in a small window, and its body scrolls when it is taller than the room.

use eframe::{
    egui::{self, RichText},
    emath::GuiRounding as _,
};

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

#[derive(Default)]
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
    /// Opens over the settings sheet: kept further in from the window's edges so it reads as
    /// a dialog on the sheet rather than lining up with it.
    pub nested: bool,
}

/// How much further in from the window a dialog over the settings sheet sits (points).
const NESTED_INSET: f32 = 14.0;
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
    let inset = if spec.nested { NESTED_INSET } else { 0.0 };
    let room = (fit::room(ctx.content_rect().size()) - chrome - egui::Vec2::splat(2.0 * inset))
        .max(egui::Vec2::ZERO);
    let id = egui::Id::unique(spec.id);
    let width = spec.width.min(room.x);
    let area = dialog_area(ctx, id, width + chrome.x, None);
    let response = egui::Modal::new(id)
        .area(area)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
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
            // why: an overlay scrollbar, so the body keeps even padding and does not shift sideways
            // when it starts to scroll (as the settings page does).
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
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

/// A one-line action bar's height: its buttons, the inset around them and the divider above.
pub(crate) fn action_bar_height(theme: &UiTheme) -> f32 {
    theme.control_height() + 2.0 * dialog::INSET + 1.0
}

/// The action bar's height as last drawn (stored under `bar`), or a single row before then.
fn bar_height(ui: &egui::Ui, theme: &UiTheme, spec: &ModalSpec<'_>, bar: egui::Id) -> f32 {
    if spec.buttons.is_empty() {
        return 0.0;
    }
    widgets::recall(ui, bar).unwrap_or_else(|| action_bar_height(theme))
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
    widgets::remember(ui, bar, ui.cursor().top() - top);
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

/// The area for a modal `outer` points wide (frame included), placed across the window from
/// that known width: halfway across, on a whole pixel. egui centres an area by the size it
/// measured the frame before, so a width that differed by a pixel from one frame to the next
/// moved the whole dialog sideways and back on every repaint (each blink of a text cursor).
/// `top` pins its top edge that far down; without it the dialog is centred down the window.
pub(crate) fn dialog_area(
    ctx: &egui::Context,
    id: egui::Id,
    outer: f32,
    top: Option<f32>,
) -> egui::Area {
    let window = ctx.content_rect();
    let ppp = ctx.pixels_per_point();
    let left = ((window.width() - outer) / 2.0)
        .max(0.0)
        .round_to_pixels(ppp);
    let area = egui::Modal::default_area(id);
    match top {
        Some(top) => area.anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(left, top.round_to_pixels(ppp)),
        ),
        None => area.anchor(egui::Align2::LEFT_CENTER, egui::vec2(left, 0.0)),
    }
}
