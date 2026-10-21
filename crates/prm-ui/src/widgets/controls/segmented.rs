//! Segmented options: flush joined buttons for a small set of choices. They keep their natural
//! widths where there is room, give up padding to fit, share a justified layout's full width,
//! and stack into a column when even their text does not fit side by side.

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::theme::UiTheme;

/// How the options of a group sit.
#[derive(Clone, Debug, PartialEq)]
enum Arrangement {
    /// Side by side, with these widths.
    Row(Vec<f32>),
    /// One above another, each this wide.
    Column(f32),
}

/// Lays out options whose text is `texts` points wide in `room` points: at their natural width
/// (`pad` each side of the text), padding shrunk towards `min_pad` when that is too wide, or as
/// a column. When `fill`, the row spans `room`, in equal widths when the widest text allows.
fn arrange(texts: &[f32], room: f32, fill: bool, (pad, min_pad): (f32, f32)) -> Arrangement {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a handful of options, far below f32's exact integer range"
    )]
    let count = texts.len().max(1) as f32;
    let text: f32 = texts.iter().sum();
    let widest = texts.iter().copied().fold(0.0, f32::max);
    let target = if fill {
        room
    } else {
        (text + 2.0 * pad * count).min(room)
    };
    if fill && target / count >= widest + 2.0 * min_pad {
        return Arrangement::Row(vec![target / count; texts.len()]);
    }
    let extra = (target - text) / count;
    if extra >= 2.0 * min_pad {
        return Arrangement::Row(texts.iter().map(|t| t + extra).collect());
    }
    let natural = widest + 2.0 * pad;
    Arrangement::Column(if fill { room } else { natural.min(room) })
}

/// Each option's rect, relative to the group's top-left corner.
fn option_rects(arrangement: &Arrangement, count: usize, height: f32) -> Vec<Rect> {
    let mut at = pos2(0.0, 0.0);
    let mut rects = Vec::with_capacity(count);
    for index in 0..count {
        let size = match arrangement {
            Arrangement::Row(widths) => vec2(widths.get(index).copied().unwrap_or(0.0), height),
            Arrangement::Column(width) => vec2(*width, height),
        };
        rects.push(Rect::from_min_size(at, size));
        match arrangement {
            Arrangement::Row(_) => at.x += size.x,
            Arrangement::Column(_) => at.y += size.y,
        }
    }
    rects
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
    let m = &theme.tokens.metrics;
    // why: measured in the bold face the selected option uses, so no choice ever clips.
    let texts: Vec<f32> = options
        .iter()
        .map(|(_, text)| {
            let font = theme.fonts.body_strong.clone();
            let color = theme.colors.text_primary;
            let galley = ui.painter().layout_no_wrap((*text).to_owned(), font, color);
            galley.size().x
        })
        .collect();
    let fill = ui.layout().horizontal_justify();
    let pads = (m.space.xl, m.space.sm);
    let arrangement = arrange(&texts, ui.available_width(), fill, pads);
    let local = option_rects(&arrangement, options.len(), theme.control_height());
    let size = local
        .iter()
        .fold(egui::Vec2::ZERO, |size, r| size.max(r.max.to_vec2()));
    let (outer, group) = ui.allocate_exact_size(size, Sense::hover());
    let rects: Vec<Rect> = local
        .iter()
        .map(|r| r.translate(outer.min.to_vec2()))
        .collect();
    let mut changed = false;
    for (index, (&(option, text), rect)) in options.iter().zip(&rects).enumerate() {
        let corners = corners(theme, &arrangement, (index, options.len()));
        let id = group.id.with(index);
        if segment(ui, theme, (id, *rect, corners), text, *value == option).clicked()
            && *value != option
        {
            *value = option;
            changed = true;
        }
    }
    paint_borders(ui, theme, outer, &rects);
    changed
}

/// An option's corners: rounded only where it meets the group's outer corners.
fn corners(
    theme: &UiTheme,
    arrangement: &Arrangement,
    (index, count): (usize, usize),
) -> CornerRadius {
    let r = theme.tokens.metrics.radius.md;
    let round = |on: bool| if on { r } else { 0 };
    let (first, last) = (index == 0, index + 1 == count);
    match arrangement {
        Arrangement::Row(_) => CornerRadius {
            nw: round(first),
            sw: round(first),
            ne: round(last),
            se: round(last),
        },
        Arrangement::Column(_) => CornerRadius {
            nw: round(first),
            ne: round(first),
            sw: round(last),
            se: round(last),
        },
    }
}

/// One option, filled to the group's border when selected.
fn segment(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (id, rect, corners): (egui::Id, Rect, CornerRadius),
    text: &str,
    selected: bool,
) -> egui::Response {
    let c = &theme.colors;
    let response = ui.interact(rect, id, Sense::click());
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

/// The group's border and the hairlines between options (vertical in a row, horizontal in a
/// column).
fn paint_borders(ui: &egui::Ui, theme: &UiTheme, outer: Rect, rects: &[Rect]) {
    let m = &theme.tokens.metrics;
    let stroke = Stroke::new(m.stroke.hairline, theme.colors.border_control);
    let painter = ui.painter();
    for pair in rects.windows(2) {
        let [before, after] = pair else {
            continue;
        };
        if (before.right() - after.left()).abs() < 0.5 {
            painter.vline(before.right(), outer.y_range(), stroke);
        } else {
            painter.hline(outer.x_range(), before.bottom(), stroke);
        }
    }
    painter.rect_stroke(
        outer,
        CornerRadius::same(m.radius.md),
        stroke,
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests;
