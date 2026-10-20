//! Controls that share the bordered button look: icon buttons, a labeled menu button, the
//! seamless search field and a text field with a flush action button.

use eframe::egui::{
    self, CornerRadius, Rect, Sense, Shape, Stroke, StrokeKind, TextEdit, pos2, vec2,
};

use crate::theme::{
    Tone, UiTheme,
    icons::{self, Icon},
};

const GLYPH: f32 = 14.0;

/// Paints a control's fill and border for its interaction state, exactly like egui buttons.
fn paint_frame(ui: &egui::Ui, rect: Rect, response: &egui::Response) {
    let v = ui.style().interact(response);
    ui.painter().rect(
        rect,
        v.corner_radius,
        v.bg_fill,
        v.bg_stroke,
        StrokeKind::Inside,
    );
}

fn hover_color(theme: &UiTheme, response: &egui::Response) -> egui::Color32 {
    if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    }
}

/// A square bordered icon button named `name` for screen readers (callers add the tooltip).
pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    icon: Icon,
    name: &str,
) -> egui::Response {
    let side = theme.tokens.metrics.row.control_height;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    paint_frame(ui, rect, &response);
    let glyph = Rect::from_center_size(rect.center(), vec2(GLYPH, GLYPH));
    icons::paint(ui.painter(), glyph, icon, hover_color(theme, &response));
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
    response
}

/// A bordered button with a leading icon and a label, filling `width` when given.
pub(crate) fn icon_text_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    icon: Icon,
    text: &str,
    width: Option<f32>,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme.fonts.button.clone(),
        theme.colors.text_primary,
    );
    let pad = m.space.md + 2.0;
    let natural = pad + GLYPH + m.space.sm + galley.size().x + pad;
    let size = vec2(width.unwrap_or(natural).max(natural), m.row.control_height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    paint_frame(ui, rect, &response);
    let glyph = Rect::from_center_size(
        pos2(rect.left() + pad + GLYPH / 2.0, rect.center().y),
        vec2(GLYPH, GLYPH),
    );
    icons::paint(ui.painter(), glyph, icon, hover_color(theme, &response));
    let at = pos2(
        glyph.right() + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    response
}

/// A bordered button reading "Label  Value ⌄" that opens a menu (callers attach the popup).
pub(crate) fn menu_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    label: &str,
    value: &str,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let font = theme.fonts.label.clone();
    let painter = ui.painter().clone();
    let label_g = painter.layout_no_wrap(label.to_owned(), font.clone(), theme.colors.text_muted);
    let value_g = painter.layout_no_wrap(value.to_owned(), font, theme.colors.text_primary);
    let (pad, gap, chevron) = (m.space.md + 2.0, m.space.sm, 8.0);
    let width = pad + label_g.size().x + gap + value_g.size().x + gap + 2.0 + chevron + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, m.row.control_height), Sense::click());
    paint_frame(ui, rect, &response);
    let y = rect.center().y;
    let mut x = rect.left() + pad;
    let label_w = label_g.size().x;
    painter.galley(
        pos2(x, y - label_g.size().y / 2.0),
        label_g,
        theme.colors.text_muted,
    );
    x += label_w + gap;
    let value_w = value_g.size().x;
    painter.galley(
        pos2(x, y - value_g.size().y / 2.0),
        value_g,
        theme.colors.text_primary,
    );
    x += value_w + gap + 2.0;
    let glyph = Rect::from_center_size(pos2(x + chevron / 2.0, y), vec2(chevron, chevron));
    icons::paint(
        &painter,
        glyph,
        Icon::ChevronDown,
        hover_color(theme, &response),
    );
    let name = format!("{label}: {value}");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
    response
}

/// A seamless filter field: a search glyph and borderless text, underlined only while focused.
pub(crate) fn search_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    (id, hint): (egui::Id, &str),
    width: f32,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let (rect, area) = ui.allocate_exact_size(vec2(width, m.row.control_height), Sense::click());
    let glyph = Rect::from_center_size(
        pos2(rect.left() + GLYPH / 2.0 + 2.0, rect.center().y),
        vec2(GLYPH, GLYPH),
    );
    let text_rect = Rect::from_min_max(pos2(glyph.right() + m.space.md, rect.top()), rect.max);
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(f32::INFINITY);
    let response = ui.put(text_rect, edit);
    if area.clicked() {
        response.request_focus();
    }
    let focused = response.has_focus();
    let color = if focused {
        theme.colors.text_primary
    } else {
        theme.colors.text_muted
    };
    icons::paint(ui.painter(), glyph, Icon::Search, color);
    let underline = if focused {
        Some(Stroke::new(m.stroke.emphasis, theme.colors.accent_base))
    } else if response.hovered() || area.hovered() {
        Some(Stroke::new(m.stroke.hairline, theme.colors.border_default))
    } else {
        None
    };
    if let Some(stroke) = underline {
        ui.painter()
            .hline(rect.x_range(), rect.bottom() - stroke.width / 2.0, stroke);
    }
    response
}

/// A bordered text field with a flush action button on its right edge.
pub(crate) struct ActionField<'a> {
    pub id: egui::Id,
    pub hint: &'a str,
    /// The button's accessible name and tooltip ("Connect").
    pub action: &'a str,
    pub icon: Icon,
    /// Draws the border in the danger tone.
    pub error: bool,
}

/// Returns true when the button is clicked or Enter is pressed in the field.
pub(crate) fn action_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    spec: &ActionField<'_>,
) -> bool {
    let m = &theme.tokens.metrics;
    let height = m.row.control_height + m.space.md;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let background = ui.painter().add(Shape::Noop);
    let button_rect = Rect::from_min_max(
        pos2(rect.right() - height - m.space.xs, rect.top()),
        rect.max,
    );
    let text_rect = Rect::from_min_max(
        pos2(rect.left() + m.space.lg, rect.top()),
        pos2(button_rect.left() - m.space.sm, rect.bottom()),
    );
    let edit = TextEdit::singleline(text)
        .id(spec.id)
        .hint_text(spec.hint)
        .frame(egui::Frame::NONE)
        .font(theme.fonts.body.clone())
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(f32::INFINITY);
    let field = ui.put(text_rect, edit);
    let button = ui.interact(button_rect, spec.id.with("action"), Sense::click());
    let border = field_border(theme, spec.error, field.has_focus());
    let radius = CornerRadius::same(m.radius.md);
    let mut shapes = vec![Shape::rect_filled(
        rect,
        radius,
        theme.colors.surface_sunken,
    )];
    if button.hovered() {
        let right = CornerRadius {
            nw: 0,
            sw: 0,
            ne: radius.ne,
            se: radius.se,
        };
        shapes.push(Shape::rect_filled(
            button_rect,
            right,
            theme.colors.control_hover,
        ));
    }
    shapes.push(Shape::rect_stroke(rect, radius, border, StrokeKind::Inside));
    let divider = button_rect.left();
    shapes.push(Shape::vline(
        divider,
        rect.y_range().shrink(border.width),
        border,
    ));
    ui.painter().set(background, Shape::Vec(shapes));
    let ready = !text.trim().is_empty();
    let arrow = if ready {
        theme.colors.accent_base
    } else {
        theme.colors.text_muted
    };
    let glyph = Rect::from_center_size(button_rect.center(), vec2(GLYPH, GLYPH));
    icons::paint(ui.painter(), glyph, spec.icon, arrow);
    let button = button.on_hover_text(spec.action);
    button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, spec.action));
    let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    button.clicked() || submitted
}

fn field_border(theme: &UiTheme, error: bool, focused: bool) -> Stroke {
    let m = &theme.tokens.metrics;
    if error {
        Stroke::new(m.stroke.emphasis, theme.colors.tone(Tone::Danger).solid)
    } else if focused {
        Stroke::new(m.stroke.emphasis, theme.colors.border_focus)
    } else {
        Stroke::new(m.stroke.hairline, theme.colors.border_control)
    }
}
