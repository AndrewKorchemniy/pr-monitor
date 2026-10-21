//! Controls that share the bordered button look: icon buttons, a labeled menu button with flush
//! menu items, settings rows (switches, segmented options), check boxes and radio buttons.

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::theme::{
    UiTheme,
    icons::{self, Icon},
};

mod rows;

pub(crate) use rows::{disclosure, row_divider, segmented, setting_row, toggle_row};

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

/// A square bordered icon button named `name` for screen readers (callers add the tooltip).
pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    icon: Icon,
    name: &str,
) -> egui::Response {
    let side = theme.control_height();
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    paint_frame(ui, rect, &response);
    let glyph = Rect::from_center_size(rect.center(), vec2(BUTTON_GLYPH, BUTTON_GLYPH));
    icons::paint(ui.painter(), glyph, icon, hover_color(theme, &response));
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
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
    let font = theme.fonts.body.clone();
    let painter = ui.painter().clone();
    let label_g = painter.layout_no_wrap(label.to_owned(), font.clone(), theme.colors.text_muted);
    let value_g = painter.layout_no_wrap(value.to_owned(), font, theme.colors.text_primary);
    let (pad, gap, chevron) = (m.space.lg, m.space.sm, 10.0);
    let width = pad + label_g.size().x + gap + value_g.size().x + gap + 2.0 + chevron + pad;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme.control_height()), Sense::click());
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

/// One row of a dropdown menu, filled edge to edge when hovered or selected. Use it in a
/// popup with no horizontal margin (`menu_frame`).
pub(crate) fn menu_item(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    text: &str,
    selected: bool,
) -> egui::Response {
    menu_item_detail(ui, theme, (text, None), MenuMark::Pointer { selected })
}

/// How a menu row shows that it is the current one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum MenuMark {
    /// A menu the pointer drives: filled while hovered; the `selected` row (the current value)
    /// is also filled, with the accent bar and a bold label.
    Pointer { selected: bool },
    /// A list the keys drive (a combobox): only the `lit` row is filled, with the accent bar.
    /// Hovering fills nothing (the caller moves the highlight instead), so one row is lit.
    Keys { lit: bool },
}

impl MenuMark {
    const fn filled(self, hovered: bool) -> bool {
        match self {
            Self::Pointer { selected } => selected || hovered,
            Self::Keys { lit } => lit,
        }
    }

    /// The accent bar, and the state screen readers hear.
    const fn current(self) -> bool {
        match self {
            Self::Pointer { selected } => selected,
            Self::Keys { lit } => lit,
        }
    }
}

/// A `menu_item` with an optional muted second text after the label (a person's sign-in
/// name), cut to fit the row.
pub(crate) fn menu_item_detail(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (text, detail): (&str, Option<&str>),
    mark: MenuMark,
) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = vec2(ui.available_width(), theme.control_height() - m.space.xs);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if mark.filled(response.hovered()) {
        ui.painter()
            .rect_filled(rect, 0.0, theme.colors.row_selected);
    }
    let current = mark.current();
    if current {
        let bar = Rect::from_min_size(rect.min, vec2(2.0, rect.height()));
        ui.painter().rect_filled(bar, 0.0, theme.colors.accent_base);
    }
    let font = if mark == (MenuMark::Pointer { selected: true }) {
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
    let after = at.x + galley.size().x + m.space.md;
    ui.painter().galley(at, galley, theme.colors.text_primary);
    if let Some(detail) = detail {
        let space = Rect::from_min_max(pos2(after, rect.top()), rect.max - vec2(m.space.lg, 0.0));
        paint_detail(ui, theme, detail, space);
    }
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::MenuItem, enabled, current, text));
    response
}

/// A muted single line, vertically centred in `space` and truncated to its width.
fn paint_detail(ui: &egui::Ui, theme: &UiTheme, detail: &str, space: Rect) {
    if space.width() <= 0.0 {
        return;
    }
    let color = theme.colors.text_muted;
    let mut job = egui::text::LayoutJob::simple_singleline(
        detail.to_owned(),
        theme.fonts.small.clone(),
        color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(space.width());
    let galley = ui.painter().layout_job(job);
    let at = pos2(space.left(), space.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, color);
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

/// The hover fill of a borderless (ghost) button, faded in and out rather than switched.
pub(crate) fn ghost_fill(ui: &egui::Ui, theme: &UiTheme, rect: Rect, response: &egui::Response) {
    let corners = CornerRadius::same(theme.tokens.metrics.radius.md);
    ghost_fill_corners(ui, theme, (rect, corners), response);
}

/// `ghost_fill` with its own corners, for a ghost button flush inside another shape.
pub(crate) fn ghost_fill_corners(
    ui: &egui::Ui,
    theme: &UiTheme,
    (rect, corners): (Rect, CornerRadius),
    response: &egui::Response,
) {
    let t = ui.ctx().animate_bool_with_time(
        response.id.with("ghost"),
        response.hovered(),
        theme.tokens.metrics.motion.quick,
    );
    if t > 0.0 {
        let fill = theme.colors.control_hover.gamma_multiply(t);
        ui.painter().rect_filled(rect, corners, fill);
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

#[cfg(test)]
mod tests;
