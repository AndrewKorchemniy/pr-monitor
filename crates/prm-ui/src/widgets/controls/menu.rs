//! Menus: the ghost button that opens one, the popup frame, and its flush rows (plain items and
//! check items) that fill edge to edge when hovered.

use eframe::egui::{self, Rect, Sense, pos2, vec2};

use super::{Check, ghost_chrome, hover_color, paint_check};
use crate::theme::{
    UiTheme,
    icons::{self, Icon},
};

/// The check box in a `menu_check` row.
const MENU_MARK: f32 = 14.0;

/// The space between a button and the menu it opens.
pub(crate) const MENU_GAP: f32 = 6.0;

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

/// One row of a multi-choice menu: a check box and its label (in the body font unless `label`
/// sets one), filled edge to edge when hovered like `menu_item`. In a disabled `Ui` it is
/// dimmed and does not respond.
pub(crate) fn menu_check(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    state: Check,
    label: egui::WidgetText,
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
    paint_check(ui, theme, mark, state, &response);
    let galley = label.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        theme.fonts.body.clone(),
    );
    let at = pos2(
        mark.right() + m.space.sm,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter()
        .galley(at, galley.clone(), theme.colors.text_primary);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::MenuItemCheckBox,
            enabled,
            state == Check::On,
            galley.text(),
        )
    });
    response
}
