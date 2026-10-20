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
        buttons: &[copy::CLOSE],
        danger: false,
        width: 560.0,
    };
    let outcome = modal(ctx, &theme, &spec, |ui| {
        egui::ScrollArea::vertical()
            .max_height(480.0)
            .show(ui, |ui| {
                for (category, title) in CATEGORIES {
                    ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
                    let grid = egui::Grid::new(("keys", title))
                        .num_columns(2)
                        .spacing([24.0, 4.0]);
                    grid.show(ui, |ui| category_rows(ui, cx, category));
                    ui.add_space(theme.tokens.metrics.space.md);
                }
            });
    });
    outcome != ModalOutcome::Open
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
