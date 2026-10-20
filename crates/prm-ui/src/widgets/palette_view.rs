//! The command palette (Ctrl+K): a query field over commands, sign-ins, themes and `!number`
//! jumps. Arrow keys move, Enter runs, Esc closes.

use eframe::egui::{self, Key, Modifiers};

use crate::{
    commands::palette::{PaletteItem, PaletteSources, PaletteState, build_items},
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{self, controls},
};

const WIDTH: f32 = 520.0;
const MAX_ROWS: usize = 12;

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
    if enter && let Some((item, _)) = items.get(state.selected) {
        return PaletteOutcome::Run(item.clone());
    }
    let mut outcome = PaletteOutcome::Open;
    let id = egui::Id::unique("palette");
    // why: a palette opens near the top, where the eye already is, not mid-window.
    let top = ctx.content_rect().height() * 0.16;
    let area = egui::Modal::default_area(id).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top));
    let modal = egui::Modal::new(id)
        .area(area)
        .frame(crate::theme::adapter::frame_popup(&theme, theme.policy))
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let field = (egui::Id::unique("palette_query"), copy::PALETTE_HINT);
            let response = controls::search_field(ui, &theme, &mut state.query, field, WIDTH);
            response.request_focus();
            if response.changed() {
                state.selected = 0;
            }
            ui.separator();
            if items.is_empty() {
                widgets::muted(ui, &theme, copy::PALETTE_EMPTY);
            }
            ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
            for (index, (item, title)) in items.iter().enumerate().take(MAX_ROWS) {
                if row(ui, &theme, title, index == state.selected).clicked() {
                    outcome = PaletteOutcome::Run(item.clone());
                }
            }
            ui.separator();
            widgets::muted(ui, &theme, copy::PALETTE_FOOTER);
        });
    if matches!(outcome, PaletteOutcome::Open) && modal.should_close() {
        outcome = PaletteOutcome::Close;
    }
    outcome
}

/// One result: left-aligned, highlighted with the accent tint when selected.
fn row(ui: &mut egui::Ui, theme: &UiTheme, title: &str, selected: bool) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = egui::vec2(WIDTH, m.row.control_height + m.space.xs);
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
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_primary,
    );
    let at = egui::pos2(
        rect.left() + m.space.md,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, theme.colors.text_primary);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, title));
    response
}
