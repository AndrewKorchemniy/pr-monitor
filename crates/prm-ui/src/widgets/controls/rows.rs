//! Settings rows: a label with a control on the right (stacked under the label when narrow), a
//! switch row, the hairline between rows and a disclosure row.

use eframe::egui::{self, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::hover_color;
use crate::{
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    widgets::fit,
};

/// The chevron's slot in a disclosure row.
const GLYPH: f32 = 14.0;

/// A label and an optional muted description on the left, `add`'s control on the right; both
/// centred vertically. The text wraps within about half the row, and the control keeps at least
/// `fit::CONTROL_WIDTH`. Narrower than `fit::ROW_STACK_BELOW`, the row stacks: the text above,
/// then the control in a justified layout, so fields, sliders and segmented options span it.
pub(crate) fn setting_row<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: (&str, Option<&str>),
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m = &theme.tokens.metrics;
    let width = ui.available_width();
    if fit::row_stacks(width) {
        return stacked_row(ui, theme, text, add);
    }
    let wrap = (width * 0.55).min(width - fit::CONTROL_WIDTH - fit::ROW_GAP);
    let block = TextBlock::new(ui, theme, text, wrap);
    // why: a full-height control (field, slider, segmented) gets the same air above and below
    // as the text does, rather than touching the row's hairlines.
    let height = (block.height + 2.0 * m.space.sm).max(theme.control_height() + 2.0 * m.space.sm);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let top = rect.center().y - block.height / 2.0;
    block.paint(ui, pos2(rect.left(), top));
    let control = Rect::from_min_max(
        pos2(rect.left() + wrap + fit::ROW_GAP, rect.top()),
        rect.max,
    );
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.scope_builder(egui::UiBuilder::new().max_rect(control).layout(layout), add)
        .inner
}

/// A narrow `setting_row`: the text across the row, then the control under it at full width.
fn stacked_row<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: (&str, Option<&str>),
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m = &theme.tokens.metrics;
    let width = ui.available_width();
    let block = TextBlock::new(ui, theme, text, width);
    ui.add_space(m.space.sm);
    let (rect, _) = ui.allocate_exact_size(vec2(width, block.height), Sense::hover());
    block.paint(ui, rect.min);
    let layout = egui::Layout::top_down(egui::Align::Min).with_cross_justify(true);
    let inner = ui
        .scope_builder(egui::UiBuilder::new().layout(layout), add)
        .inner;
    ui.add_space(m.space.sm);
    inner
}

/// A row's label and muted description, laid out to a wrap width.
struct TextBlock {
    label: std::sync::Arc<egui::Galley>,
    description: Option<std::sync::Arc<egui::Galley>>,
    gap: f32,
    height: f32,
}

impl TextBlock {
    fn new(
        ui: &egui::Ui,
        theme: &UiTheme,
        (label, description): (&str, Option<&str>),
        wrap: f32,
    ) -> Self {
        let painter = ui.painter();
        let c = &theme.colors;
        let label = painter.layout(
            label.to_owned(),
            theme.fonts.body.clone(),
            c.text_primary,
            wrap,
        );
        let description = description.map(|d| {
            painter.layout(
                d.to_owned(),
                theme.fonts.small.clone(),
                c.text_secondary,
                wrap,
            )
        });
        let gap = theme.tokens.metrics.space.xxs;
        let height = label.size().y + description.as_ref().map_or(0.0, |g| g.size().y + gap);
        Self {
            label,
            description,
            gap,
            height,
        }
    }

    fn paint(self, ui: &egui::Ui, top_left: egui::Pos2) {
        let below = top_left.y + self.label.size().y + self.gap;
        let painter = ui.painter();
        painter.galley(top_left, self.label, egui::Color32::PLACEHOLDER);
        if let Some(g) = self.description {
            painter.galley(pos2(top_left.x, below), g, egui::Color32::PLACEHOLDER);
        }
    }
}

/// One clickable row: label and optional description on the left, a switch on the right.
pub(crate) fn toggle_row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut bool,
    text: (&str, Option<&str>),
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let width = ui.available_width();
    let switch = vec2(34.0, 18.0);
    let block = TextBlock::new(ui, theme, text, (width - switch.x - m.space.xl).max(80.0));
    let height = (block.height + 2.0 * m.space.sm).max(theme.control_height());
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let top = rect.center().y - block.height / 2.0;
    block.paint(ui, pos2(rect.left(), top));
    let center = pos2(rect.right() - switch.x / 2.0, rect.center().y);
    paint_switch(
        ui,
        theme,
        Rect::from_center_size(center, switch),
        *value,
        &response,
    );
    let (enabled, on, label) = (ui.is_enabled(), *value, text.0);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, label));
    response
}

fn paint_switch(ui: &egui::Ui, theme: &UiTheme, track: Rect, on: bool, response: &egui::Response) {
    let c = &theme.colors;
    let t = ui
        .ctx()
        .animate_bool_responsive(response.id.with("switch"), on);
    let border = Stroke::new(theme.tokens.metrics.stroke.hairline, c.border_control);
    let (fill, stroke, knob) = if on {
        (c.accent_base, Stroke::NONE, c.text_on_accent)
    } else if response.hovered() {
        (c.control_hover, border, c.text_secondary)
    } else {
        (c.control_bg, border, c.text_muted)
    };
    let radius = track.height() / 2.0;
    ui.painter()
        .rect(track, radius, fill, stroke, StrokeKind::Inside);
    let x = egui::lerp((track.left() + radius)..=(track.right() - radius), t);
    ui.painter()
        .circle_filled(pos2(x, track.center().y), radius - 3.0, knob);
}

/// The hairline between settings rows.
pub(crate) fn row_divider(ui: &mut egui::Ui, theme: &UiTheme) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter().hline(rect.x_range(), rect.center().y, stroke);
}

/// A row that shows or hides extra settings, with a chevron that turns when open.
pub(crate) fn disclosure(ui: &mut egui::Ui, theme: &UiTheme, label: &str, open: &mut bool) {
    let m = &theme.tokens.metrics;
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme.fonts.label.clone(),
        theme.colors.text_secondary,
    );
    let size = vec2(GLYPH + m.space.sm + galley.size().x, m.row.control_height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *open = !*open;
    }
    let icon = if *open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    let color = hover_color(theme, &response);
    let center = pos2(rect.left() + GLYPH / 2.0, rect.center().y);
    icons::paint(
        ui.painter(),
        Rect::from_center_size(center, vec2(9.0, 9.0)),
        icon,
        color,
    );
    let at = pos2(
        rect.left() + GLYPH + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    let state = *open;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, state, label));
}
