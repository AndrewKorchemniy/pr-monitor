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
    /// The body is this tall (less in a short window) however little is in it, so what is in it
    /// can change without moving the buttons. Without it the body is as tall as its content.
    pub body_height: Option<f32>,
}

impl<'a> ModalSpec<'a> {
    /// A dialog with no buttons, as wide as usual, over the page: set the rest with a struct
    /// update (`ModalSpec { buttons, ..ModalSpec::new(id, title) }`).
    pub(crate) const fn new(id: &'a str, title: &'a str) -> Self {
        Self {
            id,
            title,
            subtitle: None,
            buttons: &[],
            danger: false,
            width: WIDTH,
            nested: false,
            body_height: None,
        }
    }
}

/// How much further in from the window a dialog over the settings sheet sits (points).
const NESTED_INSET: f32 = 14.0;
/// The usual dialog width.
pub(crate) const WIDTH: f32 = 420.0;
/// The tallest a dialog's body gets before it scrolls, however tall the window (points).
pub(crate) const MAX_BODY: f32 = 520.0;
/// The least room a body keeps in a very short window (points).
const MIN_BODY: f32 = 80.0;
/// The body height a dialog without a `body_height` is placed for (points): about a short form.
/// Taller content grows the dialog downward from where it was placed.
const NOMINAL_BODY: f32 = 160.0;
/// The header's height when placing a dialog (points): the inset round a title line.
const HEAD_HEIGHT: f32 = 80.0;

pub(crate) fn modal(
    ctx: &egui::Context,
    theme: &UiTheme,
    spec: &ModalSpec<'_>,
    body: impl FnOnce(&mut egui::Ui),
) -> ModalOutcome {
    let mut outcome = ModalOutcome::Open;
    let frame = adapter::frame_dialog(theme, theme.policy).inner_margin(egui::Margin::ZERO);
    let place = Placement::of(ctx, theme, spec, frame.total_margin().sum());
    let id = egui::Id::unique(spec.id);
    let response = show_modal(ctx, id, frame, (place.outer, place.top), |ui| {
        ui.set_width(place.width);
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
        // why: an overlay scrollbar, so the body keeps even padding and does not shift sideways
        // when it starts to scroll (as the settings page does).
        ui.spacing_mut().scroll = adapter::floating_scroll();
        body_area(id, spec.body_height, place.below - used).show(ui, |ui| {
            dialog::body(ui, theme, body);
        });
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

/// Where a dialog sits and how much room it has, from the window and the spec alone: nothing
/// in it can move it. egui places an area from the size it had the frame before, so a dialog
/// anchored by its middle moved a frame late whenever its content changed height.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Placement {
    /// The content width, in points.
    width: f32,
    /// The width with the frame around it.
    outer: f32,
    /// How far down the window the dialog's top edge sits.
    top: f32,
    /// The height from the top edge to the window's margin, less the frame: what the header,
    /// body and action bar share.
    below: f32,
}

impl Placement {
    /// `chrome` is the frame's margin and stroke on both axes.
    fn of(ctx: &egui::Context, theme: &UiTheme, spec: &ModalSpec<'_>, chrome: egui::Vec2) -> Self {
        let window = ctx.content_rect().size();
        let inset = if spec.nested { NESTED_INSET } else { 0.0 };
        let room_x = (fit::room(window).x - chrome.x - 2.0 * inset).max(0.0);
        let bar = if spec.buttons.is_empty() {
            0.0
        } else {
            action_bar_height(theme)
        };
        let body = spec.body_height.unwrap_or(NOMINAL_BODY);
        let nominal = chrome.y + HEAD_HEIGHT + body + bar;
        let margin = fit::window_margin(window.y) + inset;
        let top = fit::modal_top(window.y, nominal, margin);
        let width = spec.width.min(room_x);
        Self {
            width,
            outer: width + chrome.x,
            top,
            below: fit::room_below(window.y, top, margin) - chrome.y,
        }
    }
}

/// The body's scroll area: `fixed` points tall when given (or as tall as `room` allows), else
/// as tall as its content, up to `MAX_BODY` or `room`.
fn body_area(id: egui::Id, fixed: Option<f32>, room: f32) -> egui::ScrollArea {
    let area = egui::ScrollArea::vertical().id_salt(id.with("body"));
    match fixed {
        Some(height) => area
            .max_height(room.clamp(MIN_BODY, height))
            .auto_shrink([false, false]),
        None => area
            .max_height(room.clamp(MIN_BODY, MAX_BODY))
            .auto_shrink([false, true]),
    }
}

/// A one-line action bar's height: its buttons, the inset around them and the divider above.
fn action_bar_height(theme: &UiTheme) -> f32 {
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

/// Shows a modal in `frame`, `outer` points wide (frame included) with its top `top` points down
/// (see `dialog_area`). Every dialog, the settings window and the palette are placed through it.
pub(crate) fn show_modal<R>(
    ctx: &egui::Context,
    id: egui::Id,
    frame: egui::Frame,
    (outer, top): (f32, f32),
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    egui::Modal::new(id)
        .area(dialog_area(ctx, id, outer, top))
        .frame(frame)
        .show(ctx, add)
}

/// The area for a modal `outer` points wide (frame included), `top` points down the window:
/// halfway across, on a whole pixel. egui places an area by the size it measured the frame
/// before, so anything but its top-left corner (a width that differed by a pixel, a middle
/// that moved with the height) moved the whole dialog on every repaint (each blink of a text
/// cursor).
fn dialog_area(ctx: &egui::Context, id: egui::Id, outer: f32, top: f32) -> egui::Area {
    let ppp = ctx.pixels_per_point();
    let left = ((ctx.content_rect().width() - outer) / 2.0)
        .max(0.0)
        .round_to_pixels(ppp);
    egui::Modal::default_area(id).anchor(
        egui::Align2::LEFT_TOP,
        egui::vec2(left, top.round_to_pixels(ppp)),
    )
}

#[cfg(test)]
mod tests;
