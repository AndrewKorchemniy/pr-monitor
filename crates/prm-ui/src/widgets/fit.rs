//! How overlays and settings rows fit small windows: the margin kept around a dialog, the room
//! left for it, where a card starts, the width below which a settings row stacks its control
//! under its label, and how many check boxes fit on a line. Pure arithmetic in points, so it is
//! unit tested.

use eframe::egui::{Vec2, vec2};

/// The margin around a dialog in a phone-sized window: nearly full-bleed, so the dialog still
/// reads as a layer over the window.
const NARROW_MARGIN: f32 = 8.0;
/// The margin around a dialog in a roomy window (settings was always 64 pt smaller).
const WIDE_MARGIN: f32 = 32.0;
/// At or below this window extent (points) dialogs keep only `NARROW_MARGIN`.
const NARROW_EXTENT: f32 = 600.0;
/// At or above this window extent (points) dialogs keep `WIDE_MARGIN`; in between it grows
/// steadily, so a wider window never makes a dialog narrower.
const WIDE_EXTENT: f32 = 720.0;
/// The share of the window's height above a card that fits below it.
const CARD_TOP_SHARE: f32 = 0.16;

/// The widest settings control (a slider or a text field), in points.
pub(crate) const CONTROL_WIDTH: f32 = 280.0;
/// The gap between a settings row's label and its control, in points.
pub(crate) const ROW_GAP: f32 = 16.0;
/// The narrowest label column beside a control, in points: a few words per line.
const LABEL_MIN: f32 = 152.0;
/// Below this width (points) a settings row stacks: label and description above, the control
/// below at full width. From here up the label keeps `LABEL_MIN` beside a `CONTROL_WIDTH`
/// control.
pub(crate) const ROW_STACK_BELOW: f32 = LABEL_MIN + ROW_GAP + CONTROL_WIDTH;

/// The margin between a dialog and the window's edges along one axis, for a window `extent`
/// points long on that axis.
pub(crate) fn window_margin(extent: f32) -> f32 {
    let t = ((extent - NARROW_EXTENT) / (WIDE_EXTENT - NARROW_EXTENT)).clamp(0.0, 1.0);
    NARROW_MARGIN + (WIDE_MARGIN - NARROW_MARGIN) * t
}

/// The room a dialog has in a window of `window` points: the window less an even margin on
/// each side. Each axis takes its margin from the window's extent along it, so a taller window
/// never makes a dialog narrower.
pub(crate) fn room(window: Vec2) -> Vec2 {
    let margin = vec2(window_margin(window.x), window_margin(window.y));
    (window - 2.0 * margin).max(Vec2::ZERO)
}

/// Where a card `card` points tall starts in a window `window` points tall: a fixed share of the
/// way down (at least `floor`), so steps of different heights don't jump; higher, centred, when
/// it would not fit there; at the window's margin when it is taller than the window. `card` is
/// `None` before the card's height is known.
pub(crate) fn card_top(window: f32, card: Option<f32>, floor: f32) -> f32 {
    let margin = window_margin(window);
    let anchored = (window * CARD_TOP_SHARE).max(floor).max(margin);
    card.map_or(anchored, |card| {
        ((window - card) / 2.0).max(margin).min(anchored)
    })
}

/// `want` cut down to the room a dialog has in `window`.
pub(crate) fn fit(want: Vec2, window: Vec2) -> Vec2 {
    want.min(room(window))
}

/// A settings row `width` points wide stacks its control under its label.
pub(crate) fn row_stacks(width: f32) -> bool {
    width < ROW_STACK_BELOW
}

/// How many items `item` points wide fit side by side in `width` points with `gap` between
/// them: up to `most`, and at least one.
#[expect(
    clippy::cast_precision_loss,
    reason = "a handful of columns, far below f32's exact integer range"
)]
pub(crate) fn per_line(width: f32, (item, gap): (f32, f32), most: usize) -> usize {
    (1..=most.max(1))
        .rev()
        .find(|&n| n as f32 * (item + gap) - gap <= width)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests;
