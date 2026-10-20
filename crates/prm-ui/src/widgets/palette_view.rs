//! The command palette (Ctrl+K): a query field over commands, sign-ins, themes and `!number`
//! jumps. Arrow keys move, Enter runs, Esc closes.

use eframe::egui::{self, Key, Modifiers, RichText};

use crate::{
    commands::palette::{PaletteItem, PaletteSources, PaletteState, build_items},
    copy,
    frame::FrameCx,
    widgets,
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
    let modal = egui::Modal::new(egui::Id::unique("palette"))
        .frame(crate::theme::adapter::frame_popup(&theme, theme.policy))
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let edit = egui::TextEdit::singleline(&mut state.query)
                .hint_text(copy::PALETTE_HINT)
                .desired_width(f32::INFINITY);
            let response = ui.add(edit);
            response.request_focus();
            if response.changed() {
                state.selected = 0;
            }
            ui.separator();
            if items.is_empty() {
                widgets::muted(ui, &theme, copy::PALETTE_EMPTY);
            }
            for (index, (item, title)) in items.iter().enumerate().take(MAX_ROWS) {
                let selected = index == state.selected;
                let text = RichText::new(title).font(theme.fonts.body.clone());
                let row = ui.add_sized([WIDTH, 24.0], egui::Button::selectable(selected, text));
                if row.clicked() {
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
