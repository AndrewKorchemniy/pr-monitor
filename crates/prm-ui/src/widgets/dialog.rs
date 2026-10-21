//! The shared shape of every card and dialog: a header (title, optional subtitle, optional close
//! button), the body, and an optional action bar, separated by full-width hairlines. The frame
//! around them has no inner margin; each part pads itself so the dividers reach both edges. The
//! title wraps beside the close button, and the action bar's buttons wrap onto more lines when
//! the dialog is narrow.

use eframe::egui::{self, Margin, RichText, Stroke};

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
pub(crate) fn divider(ui: &mut egui::Ui, theme: &UiTheme) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
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
            if !head.close {
                title(ui, theme, head);
                return false;
            }
            // why: the close button is placed first, so a long title wraps beside it instead
            // of pushing it out of the dialog.
            let right = egui::Layout::right_to_left(egui::Align::Min);
            ui.with_layout(right, |ui| {
                let closed = controls::close_button(ui, theme, crate::copy::CLOSE).clicked();
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    title(ui, theme, head);
                });
                closed
            })
            .inner
        })
        .inner;
    divider(ui, theme);
    closed
}

fn title(ui: &mut egui::Ui, theme: &UiTheme, head: &DialogHead<'_>) {
    ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
    let font = if head.display {
        theme.fonts.display.clone()
    } else {
        widgets::page_title_font(theme)
    };
    ui.label(
        RichText::new(head.title)
            .font(font)
            .color(theme.colors.text_primary),
    );
    if let Some(subtitle) = head.subtitle {
        widgets::muted(ui, theme, subtitle);
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
