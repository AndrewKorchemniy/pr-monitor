//! Text inputs that share one look: a bordered field at the app's control height, the same
//! field with a flush action button, and the toolbar's seamless search field.

use eframe::egui::{
    self, CornerRadius, EventFilter, Rect, Sense, Shape, Stroke, StrokeKind, TextEdit, pos2, vec2,
};

use crate::theme::{
    Tone, UiTheme,
    icons::{self, Icon},
};

const GLYPH: f32 = 14.0;
/// The search glyph's size, and its inset from the field's left edge.
pub(crate) const SEARCH_GLYPH: f32 = GLYPH;
pub(crate) const SEARCH_GLYPH_INSET: f32 = 2.0;

/// What a bordered field shows besides its text.
#[derive(Clone, Copy)]
pub(crate) struct Field<'a> {
    pub id: egui::Id,
    pub hint: &'a str,
    /// Draws the border in the danger tone.
    pub error: bool,
    /// The widest the field gets (narrower when there is less room, and the full width of a
    /// justified layout); `None` fills the available width.
    pub width: Option<f32>,
    /// Esc stays with the field (a combobox closing its dropdown) instead of taking its focus.
    pub keeps_escape: bool,
}

impl<'a> Field<'a> {
    pub(crate) const fn new(id: egui::Id, hint: &'a str) -> Self {
        Self {
            id,
            hint,
            error: false,
            width: None,
            keeps_escape: false,
        }
    }

    pub(crate) const fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub(crate) const fn keeps_escape(mut self, keeps: bool) -> Self {
        self.keeps_escape = keeps;
        self
    }
}

/// How wide `field` is in `ui`: its own width when there is room for it, else what there is.
fn field_width(ui: &egui::Ui, field: &Field<'_>) -> f32 {
    let room = ui.available_width();
    match field.width {
        Some(width) if !ui.layout().horizontal_justify() => width.min(room),
        Some(_) | None => room,
    }
}

/// The app's standard single-line input: sunken fill, hairline border, accent border on focus.
pub(crate) fn text_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    field: &Field<'_>,
) -> egui::Response {
    let width = field_width(ui, field);
    let (rect, _) = ui.allocate_exact_size(vec2(width, theme.control_height()), Sense::hover());
    let background = ui.painter().add(Shape::Noop);
    let response = ui.put(
        text_rect(theme, rect, rect.right()),
        edit(theme, text, field),
    );
    let border = field_border(theme, field.error, response.has_focus());
    let radius = CornerRadius::same(theme.tokens.metrics.radius.md);
    ui.painter().set(
        background,
        Shape::Vec(vec![
            Shape::rect_filled(rect, radius, theme.colors.control_bg),
            Shape::rect_stroke(rect, radius, border, StrokeKind::Inside),
        ]),
    );
    response
}

fn edit<'t>(theme: &UiTheme, text: &'t mut String, field: &Field<'_>) -> TextEdit<'t> {
    let edit = TextEdit::singleline(text)
        .id(field.id)
        .hint_text(field.hint)
        .frame(egui::Frame::NONE)
        .font(theme.fonts.body.clone())
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(f32::INFINITY);
    let filter = edit.get_event_filter();
    edit.event_filter(EventFilter {
        escape: field.keeps_escape,
        ..filter
    })
}

/// Where the text sits inside a field: inset from the left border, up to `right`.
fn text_rect(theme: &UiTheme, rect: Rect, right: f32) -> Rect {
    let m = &theme.tokens.metrics;
    Rect::from_min_max(
        pos2(rect.left() + m.space.lg, rect.top()),
        pos2(right - m.space.sm, rect.bottom()),
    )
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

/// The standard field with a flush action button on its right edge, divided by a hairline.
/// Returns true when the button is clicked or Enter is pressed in the field.
pub(crate) fn action_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    (field, action, icon): (&Field<'_>, &str, Icon),
) -> bool {
    let height = theme.control_height();
    let width = field_width(ui, field);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let background = ui.painter().add(Shape::Noop);
    let button_rect = Rect::from_min_max(pos2(rect.right() - height - 4.0, rect.top()), rect.max);
    let response = ui.put(
        text_rect(theme, rect, button_rect.left()),
        edit(theme, text, field),
    );
    let button = ui.interact(button_rect, field.id.with("action"), Sense::click());
    let ready = !text.trim().is_empty();
    let border = field_border(theme, field.error, response.has_focus());
    let state = (ready, button.hovered());
    let shapes = action_frame(theme, (rect, button_rect), border, state);
    ui.painter().set(background, Shape::Vec(shapes));
    let arrow = if ready {
        theme.colors.text_on_accent
    } else {
        theme.colors.text_muted
    };
    let glyph = Rect::from_center_size(button_rect.center(), vec2(GLYPH, GLYPH));
    icons::paint(ui.painter(), glyph, icon, arrow);
    let button = button.on_hover_text(action);
    button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, action));
    let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
    let mut shapes = vec![Shape::rect_filled(rect, radius, c.control_bg)];
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

/// A seamless filter field: a search glyph and borderless text, underlined only while focused.
/// It is as wide as its hint (or its text, when longer), between `min_width` and `max_width`.
pub(crate) fn search_field(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &mut String,
    (id, hint): (egui::Id, &str),
    (min_width, max_width): (f32, f32),
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let font = theme.fonts.body.clone();
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), theme.colors.text_primary)
            .size()
            .x
    };
    let content = measure(hint).max(measure(text) + m.space.md);
    let width =
        (GLYPH + 2.0 + m.space.md + content + 24.0).clamp(min_width.min(max_width), max_width);
    let (rect, area) = ui.allocate_exact_size(vec2(width, theme.control_height()), Sense::click());
    let glyph = Rect::from_center_size(
        pos2(
            rect.left() + GLYPH / 2.0 + SEARCH_GLYPH_INSET,
            rect.center().y,
        ),
        vec2(GLYPH, GLYPH),
    );
    let clear =
        Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(20.0, 20.0));
    let text_rect = Rect::from_min_max(
        pos2(glyph.right() + m.space.md, rect.top()),
        pos2(clear.left() - m.space.xs, rect.bottom()),
    );
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .font(theme.fonts.body.clone())
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
    paint_search(ui, theme, (rect, glyph), (&response, &area));
    response
}

/// The glyph, and the underline: accent while focused, a hairline on hover, none otherwise.
fn paint_search(
    ui: &egui::Ui,
    theme: &UiTheme,
    (rect, glyph): (Rect, Rect),
    (field, area): (&egui::Response, &egui::Response),
) {
    let m = &theme.tokens.metrics;
    let focused = field.has_focus();
    let color = if focused {
        theme.colors.text_primary
    } else {
        theme.colors.text_muted
    };
    icons::paint(ui.painter(), glyph, Icon::Search, color);
    let underline = if focused {
        Some(Stroke::new(m.stroke.emphasis, theme.colors.accent_base))
    } else if field.hovered() || area.hovered() {
        Some(Stroke::new(m.stroke.hairline, theme.colors.border_default))
    } else {
        None
    };
    if let Some(stroke) = underline {
        ui.painter()
            .hline(rect.x_range(), rect.bottom() - stroke.width / 2.0, stroke);
    }
}

/// The small cross that empties a search field.
fn clear_button(ui: &mut egui::Ui, theme: &UiTheme, rect: Rect, id: egui::Id) -> egui::Response {
    let response = ui.interact(rect, id.with("clear"), Sense::click());
    let color = if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };
    icons::paint(ui.painter(), rect.shrink(4.0), Icon::Cross, color);
    let name = crate::copy::CLEAR_FILTER;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
}
