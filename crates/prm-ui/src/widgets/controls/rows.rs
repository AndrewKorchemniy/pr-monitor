//! Settings rows: a label with a control on the right, a switch row, segmented options, the
//! hairline between rows and a disclosure row.

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::hover_color;
use crate::theme::{
    UiTheme,
    icons::{self, Icon},
};

/// The chevron's slot in a disclosure row.
const GLYPH: f32 = 14.0;

/// A label and an optional muted description on the left, `add`'s control on the right; both
/// centred vertically. The text wraps within about half the row.
pub(crate) fn setting_row<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (label, description): (&str, Option<&str>),
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m = &theme.tokens.metrics;
    let width = ui.available_width();
    let wrap = (width * 0.55).min(width - 200.0).max(120.0);
    let block = TextBlock::new(ui, theme, (label, description), wrap);
    let height = (block.height + 2.0 * m.space.sm).max(theme.control_height());
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let top = rect.center().y - block.height / 2.0;
    block.paint(ui, pos2(rect.left(), top));
    let control = Rect::from_min_max(pos2(rect.left() + wrap + m.space.xl, rect.top()), rect.max);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.scope_builder(egui::UiBuilder::new().max_rect(control).layout(layout), add)
        .inner
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

/// Flush joined buttons for a small set of options: one border around the group, hairline
/// dividers between options, and the selected option filled to the borders. Each option is a
/// radio button for screen readers. Returns true when the value changed.
pub(crate) fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let font = theme.fonts.body.clone();
    let pad = theme.tokens.metrics.space.xl;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, text)| {
            let galley = ui.painter().layout_no_wrap(
                (*text).to_owned(),
                font.clone(),
                theme.colors.text_primary,
            );
            galley.size().x + 2.0 * pad
        })
        .collect();
    let size = vec2(widths.iter().sum(), theme.control_height());
    let (outer, group) = ui.allocate_exact_size(size, Sense::hover());
    let mut changed = false;
    let mut x = outer.left();
    for (index, (&(option, text), width)) in options.iter().zip(&widths).enumerate() {
        let rect = Rect::from_min_size(pos2(x, outer.top()), vec2(*width, outer.height()));
        let place = (index == 0, index + 1 == options.len());
        let id = group.id.with(index);
        if segment(ui, theme, (id, rect, place), text, *value == option).clicked()
            && *value != option
        {
            *value = option;
            changed = true;
        }
        x += width;
    }
    paint_segment_borders(ui, theme, outer, &widths);
    changed
}

/// One option: filled to the group's border when selected, rounded only on the group's ends.
fn segment(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (id, rect, (first, last)): (egui::Id, Rect, (bool, bool)),
    text: &str,
    selected: bool,
) -> egui::Response {
    let c = &theme.colors;
    let response = ui.interact(rect, id, Sense::click());
    let r = theme.tokens.metrics.radius.md;
    let corners = CornerRadius {
        nw: if first { r } else { 0 },
        sw: if first { r } else { 0 },
        ne: if last { r } else { 0 },
        se: if last { r } else { 0 },
    };
    let fill = if selected {
        c.row_selected
    } else if response.hovered() {
        c.control_hover
    } else {
        c.control_bg
    };
    ui.painter().rect_filled(rect, corners, fill);
    let (font, color) = if selected {
        (theme.fonts.body_strong.clone(), c.text_primary)
    } else {
        (theme.fonts.body.clone(), c.text_secondary)
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::RadioButton, enabled, selected, text)
    });
    response
}

/// The group's border and the hairlines between options.
fn paint_segment_borders(ui: &egui::Ui, theme: &UiTheme, outer: Rect, widths: &[f32]) {
    let m = &theme.tokens.metrics;
    let stroke = Stroke::new(m.stroke.hairline, theme.colors.border_control);
    let painter = ui.painter();
    let mut x = outer.left();
    for width in widths.iter().take(widths.len().saturating_sub(1)) {
        x += width;
        painter.vline(x, outer.y_range(), stroke);
    }
    painter.rect_stroke(
        outer,
        CornerRadius::same(m.radius.md),
        stroke,
        StrokeKind::Inside,
    );
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
