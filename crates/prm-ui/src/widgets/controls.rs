//! Shared controls: bordered and ghost (borderless) icon buttons, menus (`menu`), settings rows
//! (`rows`: switches), segmented options (`segmented`), check boxes and radio buttons.

mod menu;
mod rows;
mod segmented;

use eframe::egui::{self, CornerRadius, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

pub(crate) use self::{
    menu::{
        MENU_GAP, MENU_WIDTH, MenuMark, menu_button, menu_button_width, menu_check, menu_frame,
        menu_icon_button, menu_item, menu_item_detail,
    },
    rows::{disclosure, row_divider, setting_row, spaced_divider, toggle_row},
    segmented::segmented,
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

/// The ghost refresh button while a refresh runs: turned off (no hover fill, no clicks, no Tab
/// stop) with its arrow spinning, or holding still when animations are off. Named `name` for
/// screen readers (callers add the tooltip).
pub(crate) fn refreshing_button(ui: &mut egui::Ui, theme: &UiTheme, name: &str) -> egui::Response {
    let side = theme.control_height();
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let turn = if theme.policy.animations {
        ui.ctx().request_repaint_after(SPIN_FRAME);
        spin_angle(ui.input(|i| i.time))
    } else {
        0.0
    };
    let glyph = Rect::from_center_size(rect.center(), vec2(BUTTON_GLYPH, BUTTON_GLYPH));
    icons::paint_refresh_turned(ui.painter(), glyph, theme.colors.text_secondary, turn);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, name));
    response
}

/// How far a spinner has turned `seconds` in: once around every `SPIN_PERIOD`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an angle within one turn needs no more than f32"
)]
fn spin_angle(seconds: f64) -> f32 {
    let turns = (seconds / SPIN_PERIOD).fract();
    (turns * std::f64::consts::TAU) as f32
}

/// One full turn of a spinner, in seconds.
const SPIN_PERIOD: f64 = 1.2;
/// How often a spinner redraws: smooth at its size without a repaint every frame.
const SPIN_FRAME: std::time::Duration = std::time::Duration::from_millis(25);

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
            .rect_filled(rect, m.radius.md, ghost_overlay(theme));
    } else {
        ghost_fill(ui, theme, rect, response);
    }
    if response.has_focus() {
        let ring = Stroke::new(m.stroke.focus, theme.colors.border_focus);
        ui.painter()
            .rect_stroke(rect, m.radius.md, ring, StrokeKind::Inside);
    }
}

/// The cross in a close button (points).
pub(crate) const CLOSE_GLYPH: f32 = 14.0;

/// The side of `close_button`'s square, in points.
pub(crate) fn close_side(theme: &UiTheme) -> f32 {
    theme.control_height() - theme.tokens.metrics.space.xs
}

/// A borderless square button with a cross, for closing dialogs and drawers.
pub(crate) fn close_button(ui: &mut egui::Ui, theme: &UiTheme, name: &str) -> egui::Response {
    let side = close_side(theme);
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    ghost_fill(ui, theme, rect, &response);
    let glyph = Rect::from_center_size(rect.center(), vec2(CLOSE_GLYPH, CLOSE_GLYPH));
    icons::paint(
        ui.painter(),
        glyph,
        Icon::Cross,
        hover_color(theme, &response),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.on_hover_text(name)
}

/// A ghost button's hover and open fill: a wash of the text colour, so it shows the same on
/// the canvas, the settings sheet and dialogs. An opaque surface tone vanished on the raised
/// and overlay surfaces in the dark themes.
fn ghost_overlay(theme: &UiTheme) -> egui::Color32 {
    theme.colors.text_primary.gamma_multiply(GHOST_WASH)
}

/// How strongly `ghost_overlay` tints what is under it.
const GHOST_WASH: f32 = 0.09;

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
        let fill = ghost_overlay(theme).gamma_multiply(t);
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

/// The side of a check box or radio mark, in points.
const MARK: f32 = 16.0;

/// The width a `check`, `checkbox` or `radio` row labelled `label` takes, for laying several
/// out on a line.
pub(crate) fn check_width(ui: &egui::Ui, theme: &UiTheme, label: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, theme.colors.text_primary);
    MARK + theme.tokens.metrics.space.sm + galley.size().x
}

/// Lays out a `MARK` and a label as one clickable row; `paint` draws the mark.
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
    let size = vec2(
        MARK + m.space.sm + galley.size().x,
        galley.size().y.max(m.row.control_height - 6.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let mark_rect = Rect::from_center_size(
        pos2(rect.left() + MARK / 2.0, rect.center().y),
        vec2(MARK, MARK),
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
