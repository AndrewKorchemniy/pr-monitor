//! Shared controls: bordered and ghost (borderless) icon buttons, menus (`menu`), settings rows
//! (`rows`: switches, segmented options), check boxes and radio buttons.

mod menu;
mod rows;

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

pub(crate) use self::{
    menu::{
        MENU_GAP, MenuMark, menu_button, menu_button_width, menu_check, menu_frame,
        menu_icon_button, menu_item, menu_item_detail,
    },
    rows::{disclosure, row_divider, segmented, setting_row, toggle_row},
};
use crate::theme::{
    UiTheme,
    icons::{self, Icon},
};

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
        Chrome::Ghost => {
            let menu = egui::Popup::default_response_id(&response);
            let open = egui::Popup::is_id_open(ui.ctx(), menu);
            ghost_chrome(ui, theme, rect, &response, open);
        }
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
