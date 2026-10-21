//! The chosen people as compact pills, each with a remove button flush to its right edge.

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use prm_domain::Person;

use crate::{
    copy,
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    widgets::controls,
};

/// A pill's height, in points.
const PILL_HEIGHT: f32 = 24.0;
/// The space between a pill's left edge and its name, in points.
const PILL_PAD: f32 = 8.0;
/// The remove button's width: as wide as the pill is tall, so the hit area is square.
const REMOVE_WIDTH: f32 = PILL_HEIGHT;
/// The cross inside the remove button, in points.
const REMOVE_GLYPH: f32 = 9.0;

/// The chosen people as compact pills that wrap onto more lines.
pub(crate) fn chips(ui: &mut egui::Ui, theme: &UiTheme, people: &mut Vec<Person>) {
    let gap = theme.tokens.metrics.space.xs;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for (index, person) in people.iter().enumerate() {
            if pill(ui, theme, person) {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        people.remove(index);
    }
}

/// One person: the name (and a muted group tag), then a remove button flush to the pill's
/// right edge at its full height. True when the remove button was clicked.
fn pill(ui: &mut egui::Ui, theme: &UiTheme, person: &Person) -> bool {
    let m = &theme.tokens.metrics;
    let c = &theme.colors;
    let font = theme.fonts.label.clone();
    let name = person.display_name();
    let painter = ui.painter().clone();
    let name_g = painter.layout_no_wrap(name.to_string(), font.clone(), c.text_primary);
    let tag_g = person
        .group
        .then(|| painter.layout_no_wrap(copy::GROUP_SUFFIX.to_owned(), font, c.text_muted));
    let text_w = name_g.size().x + tag_g.as_ref().map_or(0.0, |g| g.size().x);
    let size = vec2(PILL_PAD + text_w + m.space.xs + REMOVE_WIDTH, PILL_HEIGHT);
    let (rect, label) = ui.allocate_exact_size(size, Sense::hover());
    let button_rect = Rect::from_min_max(pos2(rect.right() - REMOVE_WIDTH, rect.top()), rect.max);
    let button = ui.interact(button_rect, label.id.with("remove"), Sense::click());
    paint_pill(ui, theme, (rect, button_rect), &button);
    let y = rect.center().y - name_g.size().y / 2.0;
    let tag_x = rect.left() + PILL_PAD + name_g.size().x;
    painter.galley(pos2(rect.left() + PILL_PAD, y), name_g, c.text_primary);
    if let Some(tag) = tag_g {
        painter.galley(pos2(tag_x, y), tag, c.text_muted);
    }
    let label = match &person.unique_name {
        Some(unique) => label.on_hover_text(unique.as_ref()),
        None => label,
    };
    label.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, name.as_ref()));
    let button = button.on_hover_text(copy::REMOVE);
    button.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, copy::remove_person(&name))
    });
    button.clicked()
}

/// The pill's fill and border, and the remove button's ghost hover fill, rounded only where
/// it meets the pill's right corners. The divider between name and button is left unpainted.
fn paint_pill(
    ui: &egui::Ui,
    theme: &UiTheme,
    (rect, button): (Rect, Rect),
    response: &egui::Response,
) {
    let m = &theme.tokens.metrics;
    let c = &theme.colors;
    let radius = CornerRadius::same(m.radius.sm);
    ui.painter().rect_filled(rect, radius, c.control_bg);
    let right = CornerRadius {
        nw: 0,
        sw: 0,
        ne: radius.ne,
        se: radius.se,
    };
    controls::ghost_fill_corners(ui, theme, (button, right), response);
    let border = Stroke::new(m.stroke.hairline, c.border_control);
    ui.painter()
        .rect_stroke(rect, radius, border, StrokeKind::Inside);
    let glyph = Rect::from_center_size(button.center(), vec2(REMOVE_GLYPH, REMOVE_GLYPH));
    let color = if response.hovered() {
        c.text_primary
    } else {
        c.text_secondary
    };
    icons::paint(ui.painter(), glyph, Icon::Cross, color);
}
