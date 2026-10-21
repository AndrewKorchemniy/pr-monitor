//! The shared shape of every card and dialog: a header (title, optional subtitle, optional close
//! button), the body, and an optional action bar, separated by full-width hairlines. The frame
//! around them has no inner margin; each part pads itself so the dividers reach both edges. The
//! title wraps beside the close button, and the action bar's buttons wrap onto more lines when
//! the dialog is narrow.

use eframe::egui::{self, Margin, Rect, RichText, Stroke, UiBuilder, Vec2, pos2};

use crate::{
    theme::UiTheme,
    widgets::{self, controls},
};

/// The inset on every side of a dialog's parts, so actions sit as far from the right edge as
/// from the bottom.
pub(crate) const INSET: f32 = 20.0;

/// The header of a card or dialog.
pub(crate) struct DialogHead<'a> {
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    /// The display face for the title (the first-run wordmark).
    pub display: bool,
    /// Shows a close button.
    pub close: bool,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "spacing tokens are small whole points"
)]
fn pad(theme: &UiTheme) -> Margin {
    let inset = INSET.max(theme.tokens.metrics.space.xl);
    Margin::same(inset as i8)
}

/// A hairline across the whole card at the cursor.
/// It takes no stack spacing on either side, so `INSET` alone pads the parts it separates.
pub(crate) fn divider(ui: &mut egui::Ui, theme: &UiTheme) {
    let gap = ui.spacing().item_spacing.y;
    ui.add_space(-gap);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.add_space(-gap);
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter().hline(rect.x_range(), rect.center().y, stroke);
}

/// The title row, then a divider. Returns true when the close button was clicked.
pub(crate) fn header(ui: &mut egui::Ui, theme: &UiTheme, head: &DialogHead<'_>) -> bool {
    let closed = egui::Frame::NONE
        .inner_margin(pad(theme))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if head.close {
                let line = ui.fonts_mut(|f| f.row_height(&title_font(theme, head)));
                with_close(ui, theme, Some(line), |ui| title(ui, theme, head))
            } else {
                title(ui, theme, head);
                false
            }
        })
        .inner;
    divider(ui, theme);
    closed
}

/// `add` (a title) in the `Ui`'s width short of a close button at its right, then the button:
/// centred on the title's `first_line` (its height), or with none, halfway down the `Ui`'s rect
/// with the title centred beside it. The title wraps or is cut short instead of pushing the button out, and it comes before
/// the button in reading and Tab order. Returns true when the close button was clicked.
pub(crate) fn with_close(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    first_line: Option<f32>,
    add: impl FnOnce(&mut egui::Ui),
) -> bool {
    let side = controls::close_side(theme);
    let full = ui.available_rect_before_wrap();
    let top = match first_line {
        Some(line) => full.top() + (line - side) / 2.0,
        None => full.center().y - side / 2.0,
    };
    let close = Rect::from_min_size(pos2(full.right() - side, top), Vec2::splat(side));
    let room = full.with_max_x(close.left() - ui.spacing().item_spacing.x);
    let layout = egui::Layout::top_down(egui::Align::Min);
    ui.scope_builder(UiBuilder::new().max_rect(room).layout(layout), |ui| {
        if first_line.is_some() {
            add(ui);
        } else {
            ui.horizontal_centered(add);
        }
    });
    // why: centred on the first line the button may overhang the top inset; only its foot
    // may grow the header.
    ui.expand_to_include_y(close.bottom());
    let mut slot = ui.new_child(UiBuilder::new().max_rect(close));
    controls::close_button(&mut slot, theme, crate::copy::CLOSE).clicked()
}

fn title(ui: &mut egui::Ui, theme: &UiTheme, head: &DialogHead<'_>) {
    ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
    ui.label(
        RichText::new(head.title)
            .font(title_font(theme, head))
            .color(theme.colors.text_primary),
    );
    if let Some(subtitle) = head.subtitle {
        widgets::muted(ui, theme, subtitle);
    }
}

fn title_font(theme: &UiTheme, head: &DialogHead<'_>) -> egui::FontId {
    if head.display {
        theme.fonts.display.clone()
    } else {
        widgets::page_title_font(theme)
    }
}

/// The padded body.
pub(crate) fn body<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::NONE
        .inner_margin(pad(theme))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A divider, then the action bar: a row at control height; lay buttons out with
/// `right_to_left` (and `with_main_wrap`, so they wrap when the dialog is narrow) for the usual
/// right-aligned actions.
pub(crate) fn actions<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    divider(ui, theme);
    egui::Frame::NONE
        .inner_margin(pad(theme))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(theme.control_height());
                add(ui)
            })
            .inner
        })
        .inner
}
