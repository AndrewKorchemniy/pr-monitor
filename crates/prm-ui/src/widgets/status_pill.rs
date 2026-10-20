//! The status pill: a painted glyph (never a font glyph) and the status label, in a quiet tone.

use eframe::egui::{self, Sense, vec2};
use prm_domain::PrStatus;

use crate::theme::{Tone, UiTheme, icons::Icon};

pub(crate) const fn status_icon(status: PrStatus) -> Icon {
    match status {
        PrStatus::NeedsReview => Icon::Ring,
        PrStatus::ActiveComments => Icon::Diamond,
        PrStatus::Reviewed => Icon::Check,
    }
}

pub(crate) const fn status_tone(status: PrStatus) -> Tone {
    match status {
        PrStatus::NeedsReview => Tone::NeedsReview,
        PrStatus::ActiveComments => Tone::ActiveComments,
        PrStatus::Reviewed => Tone::Reviewed,
    }
}

const ICON: f32 = 9.0;
const GAP: f32 = 6.0;

/// Fills the cell; `compact` shows only the glyph.
pub(crate) fn status_pill(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    status: PrStatus,
    compact: bool,
) -> egui::Response {
    let colors = theme.colors.tone(status_tone(status));
    let label = status.label();
    let galley = (!compact).then(|| {
        ui.painter()
            .layout_no_wrap(label.to_owned(), theme.fonts.label.clone(), colors.fg)
    });
    let text_w = galley.as_ref().map_or(0.0, |g| g.size().x + GAP);
    let size = vec2(ICON + text_w, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + ICON / 2.0, rect.center().y),
            vec2(ICON, ICON),
        );
        crate::theme::icons::paint(ui.painter(), icon_rect, status_icon(status), colors.solid);
        if let Some(galley) = galley {
            let pos = egui::pos2(
                rect.left() + ICON + GAP,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter().galley(pos, galley, colors.fg);
        }
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, label));
    response
}
