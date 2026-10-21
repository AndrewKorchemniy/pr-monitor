//! Shared controls: bordered and ghost (borderless) icon buttons, a ghost menu button with flush
//! menu items and check items, settings rows (switches, segmented options), check boxes and
//! radio buttons.

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::theme::{
    UiTheme,
    icons::{self, Icon},
};

const GLYPH: f32 = 14.0;
/// The glyph in a square icon button.
const BUTTON_GLYPH: f32 = 18.0;

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

/// How a button draws its frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Chrome {
    /// Fill and border for every state, like egui buttons (next to fields and in forms).
    Bordered,
    /// Nothing at rest; a fading fill on hover (toolbars, so they read as text).
    Ghost,
}

/// A square bordered icon button named `name` for screen readers (callers add the tooltip).
pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    icon: Icon,
    name: &str,
) -> egui::Response {
    square_icon_button(ui, theme, (icon, name), Chrome::Bordered)
}

/// A square borderless icon button for toolbars: no chrome at rest, the ghost hover fill, and
/// a focus ring for the keyboard. Named `name` for screen readers (callers add the tooltip).
pub(crate) fn ghost_icon_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    icon: Icon,
    name: &str,
) -> egui::Response {
    square_icon_button(ui, theme, (icon, name), Chrome::Ghost)
}

fn square_icon_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (icon, name): (Icon, &str),
    chrome: Chrome,
) -> egui::Response {
    let side = theme.control_height();
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    match chrome {
        Chrome::Bordered => paint_frame(ui, rect, &response),
        Chrome::Ghost => ghost_chrome(ui, theme, rect, &response, false),
    }
    let glyph = Rect::from_center_size(rect.center(), vec2(BUTTON_GLYPH, BUTTON_GLYPH));
    icons::paint(ui.painter(), glyph, icon, hover_color(theme, &response));
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
    response
}

/// A ghost button's chrome: the hover fill (held while its menu is `open`) and a focus ring
/// when it has keyboard focus.
pub(crate) fn ghost_chrome(
    ui: &egui::Ui,
    theme: &UiTheme,
    rect: Rect,
    response: &egui::Response,
    open: bool,
) {
    let m = &theme.tokens.metrics;
    if open {
        ui.painter()
            .rect_filled(rect, m.radius.md, theme.colors.control_hover);
    } else {
        ghost_fill(ui, theme, rect, response);
    }
    if response.has_focus() {
        let ring = Stroke::new(m.stroke.focus, theme.colors.border_focus);
        ui.painter()
            .rect_stroke(rect, m.radius.md, ring, StrokeKind::Inside);
    }
}

/// A ghost button reading "Label  Value ⌄" that opens a menu (callers attach the popup with
/// `egui::Popup::menu`); it stays filled while that menu is open.
pub(crate) fn menu_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    label: &str,
    value: &str,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let font = theme.fonts.body.clone();
    let painter = ui.painter().clone();
    let label_g = painter.layout_no_wrap(label.to_owned(), font.clone(), theme.colors.text_muted);
    let value_g = painter.layout_no_wrap(value.to_owned(), font, theme.colors.text_primary);
    let (pad, gap, chevron) = (m.space.lg, m.space.sm, 10.0);
    let width = pad + label_g.size().x + gap + value_g.size().x + gap + 2.0 + chevron + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme.control_height()), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));
    ghost_chrome(ui, theme, rect, &response, open);
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

/// One row of a dropdown menu, filled edge to edge when hovered or selected. Use it in a
/// popup with no horizontal margin (`menu_frame`).
pub(crate) fn menu_item(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &str,
    selected: bool,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = vec2(ui.available_width(), theme.control_height() - m.space.xs);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, theme.colors.row_selected);
    }
    if selected {
        let bar = Rect::from_min_size(rect.min, vec2(2.0, rect.height()));
        ui.painter().rect_filled(bar, 0.0, theme.colors.accent_base);
    }
    let font = if selected {
        theme.fonts.body_strong.clone()
    } else {
        theme.fonts.body.clone()
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, theme.colors.text_primary);
    let at = pos2(
        rect.left() + m.space.lg,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::MenuItem, enabled, selected, text));
    response
}

/// The check box in a `menu_check` row.
const MENU_MARK: f32 = 14.0;

/// One row of a multi-choice menu: a check box and its label, filled edge to edge when hovered
/// like `menu_item`. In a disabled `Ui` it is dimmed and does not respond.
pub(crate) fn menu_check(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &str,
    checked: bool,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = vec2(ui.available_width(), theme.control_height() - m.space.xs);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, theme.colors.row_selected);
    }
    let center = pos2(rect.left() + m.space.lg + MENU_MARK / 2.0, rect.center().y);
    let mark = Rect::from_center_size(center, vec2(MENU_MARK, MENU_MARK));
    let state = if checked { Check::On } else { Check::Off };
    paint_check(ui, theme, mark, state, &response);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_primary,
    );
    let at = pos2(
        mark.right() + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::MenuItemCheckBox, enabled, checked, text)
    });
    response
}

/// The space between a button and the menu it opens.
pub(crate) const MENU_GAP: f32 = 6.0;

/// The popup frame for `menu_item` rows: no horizontal margin so highlights reach the edges.
#[expect(
    clippy::cast_possible_truncation,
    reason = "spacing tokens are small whole points"
)]
pub(crate) fn menu_frame(theme: &UiTheme) -> egui::Frame {
    let m = &theme.tokens.metrics;
    crate::theme::adapter::frame_popup(theme, theme.policy)
        .inner_margin(egui::Margin::symmetric(0, m.space.xs as i8))
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

/// A borderless square button with a cross, for closing dialogs and drawers.
pub(crate) fn close_button(ui: &mut egui::Ui, theme: &UiTheme, name: &str) -> egui::Response {
    let side = theme.control_height() - theme.tokens.metrics.space.xs;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    ghost_fill(ui, theme, rect, &response);
    let glyph = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
    icons::paint(
        ui.painter(),
        glyph,
        Icon::Cross,
        hover_color(theme, &response),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
}

/// A compact ghost close button for notices and chips.
pub(crate) fn small_close_button(ui: &mut egui::Ui, theme: &UiTheme, name: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    ghost_fill(ui, theme, rect, &response);
    let glyph = Rect::from_center_size(rect.center(), vec2(9.0, 9.0));
    icons::paint(
        ui.painter(),
        glyph,
        Icon::Cross,
        hover_color(theme, &response),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
}

/// The hover fill of a borderless (ghost) button, faded in and out rather than switched.
pub(crate) fn ghost_fill(ui: &egui::Ui, theme: &UiTheme, rect: Rect, response: &egui::Response) {
    let t = ui.ctx().animate_bool_with_time(
        response.id.with("ghost"),
        response.hovered(),
        theme.tokens.metrics.motion.quick,
    );
    if t > 0.0 {
        let fill = theme.colors.control_hover.gamma_multiply(t);
        ui.painter()
            .rect_filled(rect, theme.tokens.metrics.radius.md, fill);
    }
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
