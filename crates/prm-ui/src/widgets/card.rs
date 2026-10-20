//! Containers: a dialog-like card centered in the window, titled settings sections, a
//! breadcrumb trail and step dots.

use eframe::egui::{self, Rect, RichText, UiBuilder, pos2, vec2};

use crate::theme::{
    UiTheme, adapter,
    icons::{self, Icon},
};

/// Centers a bordered card of at most `width` in the available area, vertically too (from the
/// card's height last frame), and scrolls when it doesn't fit.
pub(crate) fn centered_card(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    salt: &str,
    width: f32,
    add: impl FnOnce(&mut egui::Ui),
) {
    let m = &theme.tokens.metrics;
    let id = ui.scope_id().with(("centered_card", salt));
    let viewport = ui.available_height();
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink(false)
        .show(ui, |ui| {
            let last: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(0.0);
            ui.add_space(((viewport - last) / 2.0).max(m.space.xxl));
            let width = width.min(ui.available_width() - 2.0 * m.space.xl);
            let left = ui.max_rect().center().x - width / 2.0;
            let rect =
                Rect::from_min_size(pos2(left, ui.cursor().top()), vec2(width, f32::INFINITY));
            let card = ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                adapter::frame_card(theme).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add(ui);
                })
            });
            let height = card.inner.response.rect.height();
            if (height - last).abs() > 0.5 {
                ui.data_mut(|d| d.insert_temp(id, height));
                ui.ctx().request_discard("centered card height changed");
            }
            ui.add_space(m.space.xxl);
        });
}

/// A titled group of settings: the title (and an optional muted line) above a bordered box.
pub(crate) fn section<R>(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    title: &str,
    description: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m = &theme.tokens.metrics;
    ui.label(
        RichText::new(title)
            .font(theme.fonts.body_strong.clone())
            .color(theme.colors.text_primary),
    );
    if let Some(description) = description {
        crate::widgets::muted(ui, theme, description);
    }
    ui.add_space(m.space.xs);
    let inner = adapter::frame_section(theme)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(m.space.xl);
    inner
}

/// "Organizations › contoso": muted ancestors, then the current crumb. Lays out inline, so call
/// it inside a horizontal layout.
pub(crate) fn breadcrumb(ui: &mut egui::Ui, theme: &UiTheme, crumbs: &[&str]) {
    let last = crumbs.len().saturating_sub(1);
    for (index, crumb) in crumbs.iter().enumerate() {
        let color = if index == last {
            theme.colors.text_primary
        } else {
            theme.colors.text_muted
        };
        ui.label(
            RichText::new(*crumb)
                .font(theme.fonts.title.clone())
                .color(color),
        );
        if index != last {
            icons::show(ui, Icon::ChevronRight, 9.0, theme.colors.text_muted);
        }
    }
}

/// Small dots for a multi-step flow; steps up to `current` are filled with the accent.
pub(crate) fn step_dots(ui: &mut egui::Ui, theme: &UiTheme, current: usize, total: usize) {
    let (gap, r) = (10.0, 3.0);
    #[expect(clippy::cast_precision_loss, reason = "a handful of steps")]
    let width = gap * total.saturating_sub(1) as f32 + 2.0 * r;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 2.0 * r), egui::Sense::hover());
    for step in 0..total {
        #[expect(clippy::cast_precision_loss, reason = "a handful of steps")]
        let x = rect.left() + r + gap * step as f32;
        let color = if step <= current {
            theme.colors.accent_base
        } else {
            theme.colors.border_default
        };
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), r, color);
    }
    let text = crate::copy::step_of(current + 1, total);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
}
