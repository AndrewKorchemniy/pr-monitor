//! Containers: a dialog-like card in the middle of the window, titled settings sections and a
//! breadcrumb trail.

use eframe::egui::{self, Rect, RichText, UiBuilder, pos2, vec2};

use crate::{
    theme::{
        UiTheme, adapter,
        icons::{self, Icon},
    },
    widgets::fit,
};

/// A bordered card of at most `width`, centered horizontally with its top at a fixed share of
/// the window height (so steps of different heights don't jump), higher when that would cut off
/// its foot (`fit::card_top`), and scrolling when it is taller than the window. In a narrow
/// window it keeps the same even margin as a dialog (`fit::room`).
pub(crate) fn anchored_card(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    salt: &str,
    width: f32,
    add: impl FnOnce(&mut egui::Ui),
) {
    let m = &theme.tokens.metrics;
    let window = ui.available_size();
    let width = width.min(fit::room(window).x);
    let measured = egui::Id::unique(("anchored_card_height", salt));
    let last = crate::widgets::recall(ui, measured);
    let top = fit::card_top(window.y, last, m.space.xxl);
    // why: an overlay scrollbar, so a card that scrolls stays centred between even margins.
    ui.spacing_mut().scroll = adapter::floating_scroll();
    egui::ScrollArea::vertical()
        .id_salt(("anchored_card", salt))
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(top);
            let left = ui.max_rect().center().x - width / 2.0;
            let rect =
                Rect::from_min_size(pos2(left, ui.cursor().top()), vec2(width, f32::INFINITY));
            let card = ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                let frame = adapter::frame_card(theme).inner_margin(egui::Margin::ZERO);
                frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add(ui);
                });
            });
            // why: the top depends on the card's height, so a card that changed height is laid
            // out again before it is shown in the wrong place.
            crate::widgets::remember(ui, measured, card.response.rect.height());
            ui.add_space(top.min(m.space.xxl));
        });
}

/// A titled group of settings, like a markdown section: a heading (and an optional muted line),
/// then the rows, which separate themselves with dividers.
pub(crate) fn section<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    title: &str,
    description: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m = &theme.tokens.metrics;
    ui.add_space(m.space.lg);
    ui.label(
        RichText::new(title)
            .font(theme.fonts.title.clone())
            .color(theme.colors.text_primary),
    );
    if let Some(description) = description {
        crate::widgets::muted(ui, theme, description);
    }
    ui.add_space(m.space.sm);
    let inner = add(ui);
    ui.add_space(m.space.xl);
    inner
}

/// "Organizations › contoso": muted ancestors, then the current crumb. Lays out inline, so call
/// it inside a horizontal layout; a crumb too long for the room left is cut short.
pub(crate) fn breadcrumb(ui: &mut egui::Ui, theme: &UiTheme, crumbs: &[&str]) {
    let last = crumbs.len().saturating_sub(1);
    for (index, crumb) in crumbs.iter().enumerate() {
        let color = if index == last {
            theme.colors.text_primary
        } else {
            theme.colors.text_muted
        };
        let text = RichText::new(*crumb)
            .font(crate::widgets::page_title_font(theme))
            .color(color);
        ui.add(egui::Label::new(text).truncate());
        if index != last {
            icons::show(ui, Icon::ChevronRight, 9.0, theme.colors.text_muted);
        }
    }
}
