//! The command palette (Ctrl+K): a query field over commands, sign-ins, themes and `!number`
//! jumps. Arrow keys move, Enter runs, Esc closes. In a small window it is narrower and lists
//! fewer rows at a time, scrolled to keep the highlighted one in view.

use std::ops::Range;

use eframe::egui::{self, Key, Modifiers};

use crate::{
    commands::palette::{PaletteItem, PaletteSources, PaletteState, build_items},
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{self, fields, fit},
};

/// The palette's width in a roomy window, in points.
const WIDTH: f32 = 520.0;
const MAX_ROWS: usize = 12;
/// The fewest rows it lists, however short the window.
const MIN_ROWS: usize = 3;
/// The share of the window's height above the palette.
const TOP_SHARE: f32 = 0.16;

pub(crate) enum PaletteOutcome {
    Open,
    Run(PaletteItem),
    Close,
}

pub(crate) fn palette_view(
    ctx: &egui::Context,
    cx: &FrameCx<'_>,
    state: &mut PaletteState,
    sources: &PaletteSources<'_>,
) -> PaletteOutcome {
    let theme = cx.theme.clone();
    let items = build_items(state, sources);
    if let Some(item) = keyed(ctx, state, &items) {
        return PaletteOutcome::Run(item);
    }
    let (clicked, close) = show(ctx, &theme, state, &items);
    match clicked {
        Some(item) => PaletteOutcome::Run(item),
        None if close => PaletteOutcome::Close,
        None => PaletteOutcome::Open,
    }
}

/// Up and Down move the highlight; Enter returns the highlighted item.
fn keyed(
    ctx: &egui::Context,
    state: &mut PaletteState,
    items: &[(PaletteItem, String)],
) -> Option<PaletteItem> {
    let (up, down, enter) = ctx.input_mut(|i| {
        (
            i.count_and_consume_key(Modifiers::NONE, Key::ArrowUp),
            i.count_and_consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    let last = items.len().saturating_sub(1);
    state.selected = state
        .selected
        .saturating_add(down)
        .saturating_sub(up)
        .min(last);
    enter
        .then(|| items.get(state.selected).map(|(item, _)| item.clone()))
        .flatten()
}

/// The palette near the top of the window, as wide and as tall as the window allows. Returns a
/// clicked item, and whether it should close (Esc or a click outside).
fn show(
    ctx: &egui::Context,
    theme: &UiTheme,
    state: &mut PaletteState,
    items: &[(PaletteItem, String)],
) -> (Option<PaletteItem>, bool) {
    let id = egui::Id::unique("palette");
    let window = ctx.content_rect().size();
    // why: a palette opens near the top, where the eye already is, not mid-window.
    let top = window.y * TOP_SHARE;
    let area = egui::Modal::default_area(id).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top));
    let frame = crate::theme::adapter::frame_popup(theme, theme.policy);
    let width = WIDTH.min(fit::room(window).x - frame.total_margin().sum().x);
    let rows = rows_that_fit(
        theme,
        window.y - top - fit::window_margin(window.min_elem()),
    );
    let shown = visible(state.selected, items.len(), rows);
    let mut clicked = None;
    let modal = egui::Modal::new(id)
        .area(area)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            let field = (egui::Id::unique("palette_query"), copy::PALETTE_HINT);
            let response = fields::search_field(ui, theme, &mut state.query, field, width);
            response.request_focus();
            if response.changed() {
                state.selected = 0;
            }
            ui.separator();
            if items.is_empty() {
                widgets::muted(ui, theme, copy::PALETTE_EMPTY);
            }
            ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
            for (index, (item, title)) in items.iter().enumerate().take(shown.end).skip(shown.start)
            {
                if row(ui, theme, title, index == state.selected).clicked() {
                    clicked = Some(item.clone());
                }
            }
            ui.separator();
            widgets::muted(ui, theme, copy::PALETTE_FOOTER);
        });
    let close = clicked.is_none() && modal.should_close();
    (clicked, close)
}

/// How many result rows fit in `height` points below the palette's top, besides its field,
/// separators and footer: from `MIN_ROWS` to `MAX_ROWS`.
fn rows_that_fit(theme: &UiTheme, height: f32) -> usize {
    let m = &theme.tokens.metrics;
    let chrome = 2.0 * theme.control_height() + 2.0 * m.space.xxl;
    let pitch = theme.control_height() + m.space.xxs;
    let fits = ((height - chrome) / pitch).floor().max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small, non-negative whole number of rows"
    )]
    let fits = fits as usize;
    fits.clamp(MIN_ROWS, MAX_ROWS)
}

/// The rows listed: `rows` of `count` starting at the top, moved down just enough to keep the
/// `selected` row in view.
fn visible(selected: usize, count: usize, rows: usize) -> Range<usize> {
    let start = selected.saturating_add(1).saturating_sub(rows);
    let start = start.min(count.saturating_sub(rows));
    start..(start + rows).min(count)
}

/// One result: left-aligned, highlighted with the accent tint when selected.
fn row(ui: &mut egui::Ui, theme: &UiTheme, title: &str, selected: bool) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = egui::vec2(ui.available_width(), theme.control_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected {
        Some(theme.colors.row_selected)
    } else if response.hovered() {
        Some(theme.colors.control_hover)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, m.radius.md, fill);
    }
    if selected {
        let bar = egui::Rect::from_min_size(
            rect.left_top() + egui::vec2(0.0, 6.0),
            egui::vec2(2.0, rect.height() - 12.0),
        );
        ui.painter().rect_filled(bar, 1.0, theme.colors.accent_base);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(
        title.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_primary,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * m.space.md);
    let galley = ui.painter().layout_job(job);
    let at = egui::pos2(
        rect.left() + m.space.md,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, title));
    response
}

#[cfg(test)]
mod tests;
