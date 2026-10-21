//! The keyboard shortcut sheet (?): every bound command by category, from the live keymap.

use eframe::egui::{self, RichText};

use crate::{
    commands::{COMMANDS, Category},
    copy,
    frame::FrameCx,
    widgets::{
        self,
        modal::{ModalOutcome, ModalSpec, modal},
    },
};

const CATEGORIES: [(Category, &str); 6] = [
    (Category::General, "General"),
    (Category::View, "View"),
    (Category::Navigate, "Navigate"),
    (Category::PullRequest, "Pull requests"),
    (Category::Notifications, "Notifications"),
    (Category::Window, "Window"),
];

/// Returns true when the sheet should close.
pub(crate) fn shortcuts(ctx: &egui::Context, cx: &FrameCx<'_>) -> bool {
    let theme = cx.theme.clone();
    let spec = ModalSpec {
        id: "shortcuts",
        title: copy::SHORTCUTS_TITLE,
        subtitle: None,
        buttons: &[copy::CLOSE],
        danger: false,
        width: 560.0,
    };
    let outcome = modal(ctx, &theme, &spec, |ui| {
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 240.0).clamp(160.0, 480.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                key_grid(ui, cx);
            });
    });
    outcome != ModalOutcome::Open
}

/// Every category in one grid, so all key caps line up in one column.
fn key_grid(ui: &mut egui::Ui, cx: &FrameCx<'_>) {
    let theme = &cx.theme;
    let grid = egui::Grid::new("keys")
        .num_columns(2)
        .min_col_width(280.0)
        .spacing([24.0, 6.0]);
    grid.show(ui, |ui| {
        for (category, title) in CATEGORIES {
            ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
            ui.end_row();
            category_rows(ui, cx, category);
        }
    });
}

/// One row per bound command: its title and every chord.
fn category_rows(ui: &mut egui::Ui, cx: &FrameCx<'_>, category: Category) {
    for spec in COMMANDS.iter().filter(|c| c.category == category) {
        let chords: Vec<String> = cx
            .keymap
            .bindings()
            .iter()
            .filter(|b| b.command == spec.id)
            .map(|b| b.chord.to_string())
            .collect();
        if chords.is_empty() {
            continue;
        }
        ui.label(spec.title);
        widgets::kbd_row(ui, &cx.theme, &chords);
        ui.end_row();
    }
}
