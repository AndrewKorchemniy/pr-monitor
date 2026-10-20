//! Controls that share the bordered button look: icon buttons, a labeled menu button, the
//! seamless search field, a text field with a flush action button, and settings rows
//! (switches, sliders, segmented options).

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
    let clear = Rect::from_center_size(
        pos2(rect.right() - m.space.sm - 10.0, rect.center().y),
        vec2(20.0, 20.0),
    );
    let right = if text.is_empty() {
        rect.right()
    } else {
        clear.left() - m.space.xs
    };
    let text_rect = Rect::from_min_max(
        pos2(glyph.right() + m.space.md, rect.top()),
        pos2(right, rect.bottom()),
    );
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(f32::INFINITY);
    let mut response = ui.put(text_rect, edit);
    if area.clicked() {
        response.request_focus();
    }
    if !text.is_empty() && clear_button(ui, theme, clear, id).clicked() {
        text.clear();
        response.mark_changed();
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

/// The small cross that empties a search field.
fn clear_button(ui: &mut egui::Ui, theme: &UiTheme, rect: Rect, id: egui::Id) -> egui::Response {
    let response = ui.interact(rect, id.with("clear"), Sense::click());
    let color = hover_color(theme, &response);
    icons::paint(ui.painter(), rect.shrink(6.0), Icon::Cross, color);
    let name = crate::copy::CLEAR_FILTER;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
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
    /// The larger first-run size; dialogs use the regular control height.
    pub tall: bool,
}

/// Returns true when the button is clicked or Enter is pressed in the field.
pub(crate) fn action_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    spec: &ActionField<'_>,
) -> bool {
    let m = &theme.tokens.metrics;
    let height = if spec.tall {
        m.row.control_height + m.space.md
    } else {
        m.row.control_height
    };
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
    let ready = !text.trim().is_empty();
    let border = field_border(theme, spec.error, field.has_focus());
    let shapes = action_frame(
        theme,
        (rect, button_rect),
        border,
        (ready, button.hovered()),
    );
    ui.painter().set(background, Shape::Vec(shapes));
    let arrow = if ready {
        theme.colors.text_on_accent
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

/// The field's fill and border, the button's fill (accent once there is text) and the divider.
fn action_frame(
    theme: &UiTheme,
    (rect, button): (Rect, Rect),
    border: Stroke,
    (ready, hovered): (bool, bool),
) -> Vec<Shape> {
    let c = &theme.colors;
    let radius = CornerRadius::same(theme.tokens.metrics.radius.md);
    let mut shapes = vec![Shape::rect_filled(rect, radius, c.surface_sunken)];
    let fill = match (ready, hovered) {
        (true, true) => Some(c.accent_hover),
        (true, false) => Some(c.accent_base),
        (false, true) => Some(c.control_hover),
        (false, false) => None,
    };
    if let Some(fill) = fill {
        let right = CornerRadius {
            nw: 0,
            sw: 0,
            ne: radius.ne,
            se: radius.se,
        };
        shapes.push(Shape::rect_filled(button, right, fill));
    }
    shapes.push(Shape::rect_stroke(rect, radius, border, StrokeKind::Inside));
    let divider = rect.y_range().shrink(border.width);
    shapes.push(Shape::vline(button.left(), divider, border));
    shapes
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

/// A slider with a visible rail, an accent fill up to the value and a round knob. The caller
/// states the value once, in the row's label.
pub(crate) fn slider<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    name: &str,
) -> egui::Response {
    ui.scope(|ui| {
        let style = ui.style_mut();
        let c = &theme.colors;
        style.visuals.widgets.inactive.bg_fill = c.border_default;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(2.0, c.accent_base);
        style.visuals.selection.bg_fill = c.accent_base;
        style.spacing.interact_size.y = 16.0;
        style.spacing.slider_width = 180.0;
        style.spacing.slider_rail_height = 4.0;
        let slider = egui::Slider::new(value, range)
            .show_value(false)
            .trailing_fill(true)
            .handle_shape(egui::style::HandleShape::Circle);
        let response = ui.add(slider);
        let enabled = ui.is_enabled();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, enabled, name));
        response
    })
    .inner
}

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
    let height = (block.height + 2.0 * m.space.sm).max(m.row.control_height + m.space.md);
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
    let height = (block.height + 2.0 * m.space.sm).max(m.row.control_height + m.space.md);
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

/// Joined segments for a small set of options; each segment is a radio button for screen
/// readers. Returns true when the value changed.
pub(crate) fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let m = &theme.tokens.metrics;
    let background = ui.painter().add(Shape::Noop);
    let mut changed = false;
    let mut outer = Rect::NOTHING;
    // why: a right-to-left row places items from the right, so feed the options backwards to
    // keep them reading left to right.
    let reversed = ui.layout().prefer_right_to_left();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let ordered: Vec<_> = if reversed {
            options.iter().rev().collect()
        } else {
            options.iter().collect()
        };
        for &&(option, text) in &ordered {
            let selected = *value == option;
            let (rect, response) = segment(ui, theme, text, selected);
            outer = outer.union(rect);
            if response.clicked() && !selected {
                *value = option;
                changed = true;
            }
        }
    });
    let radius = CornerRadius::same(m.radius.md);
    let border = Stroke::new(m.stroke.hairline, theme.colors.border_subtle);
    let shapes = vec![
        Shape::rect_filled(outer, radius, theme.colors.control_bg),
        Shape::rect_stroke(outer, radius, border, StrokeKind::Inside),
    ];
    ui.painter().set(background, Shape::Vec(shapes));
    changed
}

fn segment(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &str,
    selected: bool,
) -> (Rect, egui::Response) {
    let m = &theme.tokens.metrics;
    let c = &theme.colors;
    let color = if selected {
        c.text_primary
    } else {
        c.text_secondary
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme.fonts.label.clone(), color);
    let size = vec2(galley.size().x + 2.0 * m.space.lg, m.row.control_height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let inner = rect.shrink(3.0);
    if selected {
        let border = Stroke::new(m.stroke.hairline, c.border_default);
        ui.painter().rect(
            inner,
            m.radius.sm,
            c.surface_raised,
            border,
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(inner, m.radius.sm, c.control_hover);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::RadioButton, enabled, selected, text)
    });
    (rect, response)
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

/// A borderless square button with a cross, for closing dialogs and drawers.
pub(crate) fn close_button(ui: &mut egui::Ui, theme: &UiTheme, name: &str) -> egui::Response {
    let m = &theme.tokens.metrics;
    let side = m.row.control_height - 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, m.radius.md, theme.colors.control_hover);
    }
    let glyph = Rect::from_center_size(rect.center(), vec2(10.0, 10.0));
    icons::paint(
        ui.painter(),
        glyph,
        Icon::Cross,
        hover_color(theme, &response),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
}

/// A check box's state; `Mixed` is a project with only some repositories chosen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Check {
    Off,
    On,
    Mixed,
}

/// A themed check box with its label; the whole row is clickable.
pub(crate) fn check(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    state: Check,
    label: egui::WidgetText,
) -> egui::Response {
    mark_row(
        ui,
        theme,
        label,
        |ui, rect, response| {
            paint_check(ui, theme, rect, state, response);
        },
        (egui::Role::CheckBox, state == Check::On),
    )
}

/// A check box bound to a bool; returns a response that is changed when toggled.
pub(crate) fn checkbox(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    value: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let state = if *value { Check::On } else { Check::Off };
    let mut response = check(ui, theme, state, label.into());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response
}

/// A themed radio button bound to `current`; selects `option` when clicked.
pub(crate) fn radio<T: PartialEq>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    current: &mut T,
    option: T,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let selected = *current == option;
    let mut response = mark_row(
        ui,
        theme,
        label.into(),
        |ui, rect, response| {
            paint_radio(ui, theme, rect, selected, response);
        },
        (egui::Role::RadioButton, selected),
    );
    if response.clicked() && !selected {
        *current = option;
        response.mark_changed();
    }
    response
}

/// Lays out a 16 pt mark and a label as one clickable row; `paint` draws the mark.
fn mark_row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    label: egui::WidgetText,
    paint: impl FnOnce(&egui::Ui, Rect, &egui::Response),
    (role, on): (egui::Role, bool),
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let galley = label.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let mark = 16.0;
    let size = vec2(
        mark + m.space.sm + galley.size().x,
        galley.size().y.max(m.row.control_height - 6.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let mark_rect = Rect::from_center_size(
        pos2(rect.left() + mark / 2.0, rect.center().y),
        vec2(mark, mark),
    );
    paint(ui, mark_rect, &response);
    let text = galley.text().to_owned();
    let at = pos2(
        mark_rect.right() + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::selected(role, enabled, on, &text));
    response
}

fn paint_check(
    ui: &egui::Ui,
    theme: &UiTheme,
    rect: Rect,
    state: Check,
    response: &egui::Response,
) {
    let c = &theme.colors;
    let radius = theme.tokens.metrics.radius.sm;
    let ink = Stroke::new(2.0, c.text_on_accent);
    match state {
        Check::Off => {
            let fill = if response.hovered() {
                c.control_hover
            } else {
                c.control_bg
            };
            let border = Stroke::new(1.0, c.border_control);
            ui.painter()
                .rect(rect, radius, fill, border, StrokeKind::Inside);
        }
        Check::On => {
            ui.painter().rect_filled(rect, radius, c.accent_base);
            icons::paint_stroke(ui.painter(), rect.shrink(3.0), Icon::Check, ink);
        }
        Check::Mixed => {
            ui.painter().rect_filled(rect, radius, c.accent_base);
            let y = rect.center().y;
            ui.painter()
                .hline((rect.left() + 4.0)..=(rect.right() - 4.0), y, ink);
        }
    }
}

fn paint_radio(
    ui: &egui::Ui,
    theme: &UiTheme,
    rect: Rect,
    selected: bool,
    response: &egui::Response,
) {
    let c = &theme.colors;
    let center = rect.center();
    let r = rect.width() / 2.0;
    if selected {
        ui.painter().circle_filled(center, r, c.accent_base);
        ui.painter()
            .circle_filled(center, r * 0.4, c.text_on_accent);
    } else {
        let fill = if response.hovered() {
            c.control_hover
        } else {
            c.control_bg
        };
        ui.painter()
            .circle(center, r - 0.5, fill, Stroke::new(1.0, c.border_control));
    }
}
