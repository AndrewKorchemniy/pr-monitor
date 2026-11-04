//! A contained slider: the rail, its accent fill and the knob sit inside a bordered field with
//! the value at the right edge. Drag or click the rail, use the arrow keys, or double-click the
//! value to type one. It shrinks to fit narrow rows and spans a justified layout.

use std::{ops::RangeInclusive, sync::Arc};

use eframe::egui::{
    self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, emath::Numeric, pos2,
    vec2,
};

use crate::{
    theme::UiTheme,
    widgets::fields::{bare_edit, keep_escape},
};

const VALUE_WIDTH: f32 = 64.0;
/// How far the value's right edge sits inside the field's right border (points).
const VALUE_INSET: f32 = 12.0;
/// The usual width of a settings slider.
pub(crate) const SLIDER_WIDTH: f32 = crate::widgets::fit::CONTROL_WIDTH;
const KNOB: f32 = 7.0;

/// How a slider is named and how its value reads.
pub(crate) struct SliderSpec<'a> {
    /// A stable id, so a drag survives layout changes elsewhere on the page.
    pub id: egui::Id,
    pub name: &'a str,
    /// The shown value is the stored one times `scale` (100 for a 1.0 = 100% scale).
    pub scale: f64,
    /// Appended to the shown value ("min", "%").
    pub unit: &'a str,
    /// The widest it gets; narrower when the row is, and the full width of a justified layout.
    pub width: f32,
}

/// Returns a response that is `changed()` when the value moved.
pub(crate) fn slider_field<N: Numeric>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut N,
    range: RangeInclusive<N>,
    spec: &SliderSpec<'_>,
) -> egui::Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let width = if ui.layout().horizontal_justify() {
        ui.available_width()
    } else {
        spec.width.min(ui.available_width())
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, theme.control_height()), Sense::hover());
    let value_rect = value_slot(rect);
    let rail = Rect::from_min_max(
        pos2(rect.left() + KNOB + 8.0, rect.top()),
        pos2(value_rect.left() - KNOB - 4.0, rect.bottom()),
    );
    // why: the field is painted after the value editor runs, so reserve its slot underneath.
    let background = ui.painter().add(egui::Shape::Noop);
    let before = value.to_f64();
    let mut v = before;
    let mut response = drag_rail(ui, rail, spec.id, (lo, hi), &mut v);
    step_keys(ui, &response, (lo, hi), N::INTEGRAL, &mut v);
    let typed = edit_value(ui, theme, value_rect, spec, &mut v);
    response |= typed;
    v = v.clamp(lo, hi);
    if N::INTEGRAL {
        v = v.round();
    }
    if (v - before).abs() > f64::EPSILON {
        *value = N::from_f64(v);
        response.mark_changed();
    }
    let fraction = fraction_of(v, (lo, hi));
    let editing = ui.data(|d| d.get_temp::<String>(spec.id.with("editing")).is_some());
    let text = shown(v, spec);
    let focused = response.has_focus() || editing;
    paint(
        ui,
        theme,
        (rect, rail, background),
        (fraction, focused),
        &response,
    );
    if !editing {
        paint_galley(ui, theme, value_rect, value_galley(ui, theme, &text));
    }
    let (enabled, name) = (ui.is_enabled(), spec.name);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Slider, enabled, name);
        info.value = Some(v);
        info.current_text_value = Some(text.clone());
        info
    });
    response
}

fn shown(v: f64, spec: &SliderSpec<'_>) -> String {
    format!("{:.0}{}", v * spec.scale, spec.unit)
}

/// Clicking or dragging anywhere on the rail moves the knob to the pointer.
fn drag_rail(
    ui: &mut egui::Ui,
    rail: Rect,
    id: egui::Id,
    (lo, hi): (f64, f64),
    v: &mut f64,
) -> egui::Response {
    let hit = rail.expand2(vec2(KNOB + 4.0, 0.0));
    let response = ui.interact(hit, id, Sense::click_and_drag());
    if (response.dragged() || response.clicked() || response.is_pointer_button_down_on())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        response.request_focus();
        let t = ((pointer.x - rail.left()) / rail.width().max(1.0)).clamp(0.0, 1.0);
        *v = lo + f64::from(t) * (hi - lo);
    }
    response
}

/// Arrow keys move by one step (one unit, or a hundredth of the range for fractions).
fn step_keys(
    ui: &egui::Ui,
    response: &egui::Response,
    (lo, hi): (f64, f64),
    integral: bool,
    v: &mut f64,
) {
    if !response.has_focus() {
        return;
    }
    let step = if integral { 1.0 } else { (hi - lo) / 100.0 };
    let (down, up) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
        )
    });
    if down {
        *v -= step;
    }
    if up {
        *v += step;
    }
}

/// The editor a slider value is being typed in, left in egui's data for `value_edit_active`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ActiveEditor(egui::Id);

/// Whether a slider's value is being typed, so Esc belongs to the editor (it cancels the edit)
/// and the app must not act on it.
pub(crate) fn value_edit_active(ctx: &egui::Context) -> bool {
    let editor: Option<ActiveEditor> = ctx.data(|d| d.get_temp(egui::Id::NULL));
    editor.is_some_and(|ActiveEditor(id)| ctx.memory(|m| m.has_focus(id)))
}

/// Double-click the value to type a number; Enter or clicking away applies it, Esc cancels.
/// The editor takes the digits' place and the unit stays where it was, so nothing moves.
fn edit_value(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    rect: Rect,
    spec: &SliderSpec<'_>,
    v: &mut f64,
) -> egui::Response {
    let key = spec.id.with("editing");
    let value = ui.interact(rect, spec.id.with("value"), Sense::click());
    let mut buffer: Option<String> = ui.data(|d| d.get_temp(key));
    let starting = buffer.is_none() && value.double_clicked();
    if starting {
        buffer = Some(format!("{:.0}", *v * spec.scale));
    }
    let Some(mut text) = buffer else {
        return value.on_hover_text(crate::copy::DOUBLE_CLICK_TO_TYPE);
    };
    let unit = value_galley(ui, theme, spec.unit);
    let edit = bare_edit(
        theme,
        &mut text,
        spec.id.with("edit"),
        egui::Align2::RIGHT_CENTER,
    );
    // why: Esc cancels the edit; egui would otherwise take the focus away before we see it.
    let edit = keep_escape(edit, true);
    let field = ui
        .put(edit_rect(rect, unit.size().x), edit)
        .accessible_name(spec.name);
    paint_galley(ui, theme, rect, unit);
    if starting {
        field.request_focus();
    }
    let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    // why: an editor that is on screen without focus (it was hidden mid-edit) is over too.
    if field.lost_focus() || cancel || (!starting && !field.has_focus()) {
        if !cancel && let Some(typed) = parse_typed(&text, spec) {
            *v = typed;
        }
        ui.data_mut(|d| {
            d.remove::<String>(key);
            // why: only this editor's own claim, never another slider's.
            if d.get_temp(egui::Id::NULL) == Some(ActiveEditor(field.id)) {
                d.remove::<ActiveEditor>(egui::Id::NULL);
            }
        });
        field.surrender_focus();
    } else {
        ui.data_mut(|d| {
            d.insert_temp(key, text);
            d.insert_temp(egui::Id::NULL, ActiveEditor(field.id));
        });
    }
    value | field
}

/// The stored value a typed `text` stands for, if it reads as a number (a pasted unit is fine).
fn parse_typed(text: &str, spec: &SliderSpec<'_>) -> Option<f64> {
    let digits = text.trim().trim_end_matches(spec.unit).trim();
    let typed: f64 = digits.parse().ok()?;
    typed.is_finite().then_some(typed / spec.scale)
}

/// The editor's rect: the value slot's full height (the input's own minimum, so it never sits
/// off-centre), ending where the unit begins.
fn edit_rect(slot: Rect, unit_width: f32) -> Rect {
    let right = (slot.right() - VALUE_INSET - unit_width).max(slot.left());
    Rect::from_min_max(slot.min, pos2(right, slot.bottom()))
}

/// The value's slot: the field's right `VALUE_WIDTH` points, at its full height.
fn value_slot(field: Rect) -> Rect {
    Rect::from_min_max(pos2(field.right() - VALUE_WIDTH, field.top()), field.max)
}

/// Where a galley of `size` sits in the value slot: flush right at the inset, centred.
fn value_pos(slot: Rect, size: Vec2) -> Pos2 {
    pos2(
        slot.right() - VALUE_INSET - size.x,
        slot.center().y - size.y / 2.0,
    )
}

/// Where `v` sits between `lo` and `hi`, from 0 to 1.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a 0..=1 fraction for drawing"
)]
fn fraction_of(v: f64, (lo, hi): (f64, f64)) -> f32 {
    if hi > lo {
        ((v - lo) / (hi - lo)) as f32
    } else {
        0.0
    }
}

/// The field, the rail with its accent fill, and the accent knob.
fn paint(
    ui: &egui::Ui,
    theme: &UiTheme,
    (rect, rail, background): (Rect, Rect, egui::layers::ShapeIdx),
    (fraction, focused): (f32, bool),
    response: &egui::Response,
) {
    let colors = &theme.colors;
    let metrics = &theme.tokens.metrics;
    let border = if focused {
        Stroke::new(metrics.stroke.emphasis, colors.border_focus)
    } else {
        Stroke::new(metrics.stroke.hairline, colors.border_control)
    };
    let painter = ui.painter();
    let corners = CornerRadius::same(metrics.radius.md);
    painter.set(
        background,
        egui::Shape::Vec(vec![
            egui::Shape::rect_filled(rect, corners, colors.control_bg),
            egui::Shape::rect_stroke(rect, corners, border, StrokeKind::Inside),
        ]),
    );
    let mid = rect.center().y;
    let knob_x = egui::lerp(rail.left()..=rail.right(), fraction);
    let bar = |from: f32, to: f32, color: Color32| {
        let shape = Rect::from_min_max(pos2(from, mid - 2.0), pos2(to, mid + 2.0));
        painter.rect_filled(shape, CornerRadius::same(metrics.radius.xs), color);
    };
    bar(rail.left(), rail.right(), colors.border_default);
    // why: under the knob a short fill read as a stray dot beside it.
    if knob_x - rail.left() >= KNOB {
        bar(rail.left(), knob_x, colors.accent_base);
    }
    let active = response.hovered() || response.dragged();
    let size = if active { KNOB + 1.0 } else { KNOB };
    painter.circle_filled(pos2(knob_x, mid), size, colors.accent_base);
}

fn value_galley(ui: &egui::Ui, theme: &UiTheme, text: &str) -> Arc<egui::Galley> {
    ui.painter().layout_no_wrap(
        text.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_primary,
    )
}

fn paint_galley(ui: &egui::Ui, theme: &UiTheme, slot: Rect, galley: Arc<egui::Galley>) {
    let at = value_pos(slot, galley.size());
    ui.painter().galley(at, galley, theme.colors.text_primary);
}

#[cfg(test)]
mod tests;
