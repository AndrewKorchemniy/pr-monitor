//! The command palette (Ctrl+K): a query field over commands, sign-ins, themes and `!number`
//! jumps. Arrow keys move, Enter runs, Esc closes. It lists up to a dozen rows at a time (fewer
//! in a short window) and scrolls for the rest; the highlighted row is scrolled into view when
//! it moves. In a narrow window it is narrower.

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
    let frame = crate::theme::adapter::frame_popup(theme, theme.policy);
    let chrome = frame.total_margin().sum().x;
    let width = WIDTH.min(fit::room(window).x - chrome);
    let area = widgets::modal::dialog_area(ctx, id, width + chrome, Some(top));
    let rows = rows_that_fit(theme, window.y - top - fit::window_margin(window.y));
    let mut clicked = None;
    let modal = egui::Modal::new(id)
        .area(area)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            let field = (egui::Id::unique("palette_query"), copy::PALETTE_HINT);
            let response = fields::search_field(ui, theme, &mut state.query, field, (width, width));
            response.request_focus();
            if response.changed() {
                state.selected = 0;
                state.revealed = None;
            }
            rule(ui, theme);
            if items.is_empty() {
                widgets::muted(ui, theme, copy::PALETTE_EMPTY);
            }
            let margin = frame.inner_margin.left.into();
            clicked = list(ui, theme, state, items, (rows, margin));
            rule(ui, theme);
            widgets::muted(ui, theme, copy::PALETTE_FOOTER);
        });
    let close = clicked.is_none() && modal.should_close();
    (clicked, close)
}

/// A hairline with the same space above and below it.
fn rule(ui: &mut egui::Ui, theme: &UiTheme) {
    let space = theme.tokens.metrics.space.sm;
    ui.add_space(space);
    widgets::dialog::divider(ui, theme);
    ui.add_space(space);
}

/// The results, `rows` high at most, scrolling (by wheel or bar) for the rest; the highlighted
/// row is scrolled into view each time it moves. The rows reach `margin` past the content on
/// each side, so highlights run edge to edge like the menus'. Returns a clicked item.
fn list(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    state: &mut PaletteState,
    items: &[(PaletteItem, String)],
    (rows, margin): (usize, f32),
) -> Option<PaletteItem> {
    let mut clicked = None;
    let wide = ui
        .available_rect_before_wrap()
        .expand2(egui::vec2(margin, 0.0));
    let mut list = ui.new_child(egui::UiBuilder::new().max_rect(wide));
    list.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
    // why: an overlay scrollbar, so the rows keep their width once the list scrolls.
    list.spacing_mut().scroll = crate::theme::adapter::floating_scroll();
    egui::ScrollArea::vertical()
        .id_salt("palette_rows")
        .max_height(list_height(theme, rows, items.len()))
        .auto_shrink([false, true])
        .show(&mut list, |ui| {
            for (index, (item, title)) in items.iter().enumerate() {
                let selected = index == state.selected;
                let response = row(ui, theme, title, (selected, margin));
                if selected && state.revealed != Some(index) {
                    response.scroll_to_me(None);
                    state.revealed = Some(index);
                }
                if response.clicked() {
                    clicked = Some(item.clone());
                }
            }
        });
    // why: a child `Ui` does not move its parent's cursor; the part past the margins is the
    // frame's, so it must not widen the palette.
    ui.advance_cursor_after_rect(list.min_rect().shrink2(egui::vec2(margin, 0.0)));
    clicked
}

/// The height of `rows` result rows and the gaps between them, in points. When there are more
/// than `rows` of `total`, half of the next row shows, so the list reads as scrollable.
fn list_height(theme: &UiTheme, rows: usize, total: usize) -> f32 {
    let gap = theme.tokens.metrics.space.xxs;
    #[expect(clippy::cast_precision_loss, reason = "a dozen rows")]
    let shown = if total > rows {
        rows as f32 - 0.5
    } else {
        rows as f32
    };
    shown * (theme.control_height() + gap) - gap
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

/// One result, highlighted edge to edge with the accent bar when selected, like a menu row;
/// its text sits `margin` in, under the query's.
fn row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    title: &str,
    (selected, margin): (bool, f32),
) -> egui::Response {
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
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    if selected {
        let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height()));
        ui.painter().rect_filled(bar, 0.0, theme.colors.accent_base);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(
        title.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_primary,
    );
    let inset = margin + m.space.md;
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * inset);
    let galley = ui.painter().layout_job(job);
    let at = egui::pos2(rect.left() + inset, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, theme.colors.text_primary);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, title));
    response
}

#[cfg(test)]
mod tests;
