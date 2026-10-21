//! The keyboard shortcut sheet (?): every bound command by category, from the live keymap. A
//! grid with the key caps in one column, or in a narrow window a list with each command's keys
//! under its title.

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
    // why: the dialog's body scrolls, so the list needs no scroll area of its own.
    let outcome = modal(ctx, &theme, &spec, |ui| {
        if ui.available_width() < COLUMN + GAP + KEYS {
            key_list(ui, cx);
        } else {
            key_grid(ui, cx);
        }
    });
    outcome != ModalOutcome::Open
}

/// The width of the grid's column of commands, in points.
const COLUMN: f32 = 280.0;
/// The room the grid keeps for a command's key caps (two chords), in points.
const KEYS: f32 = 200.0;
/// The space between the grid's columns, in points.
const GAP: f32 = 24.0;

/// Every category in one grid, so all key caps line up in one column.
fn key_grid(ui: &mut egui::Ui, cx: &FrameCx<'_>) {
    let theme = &cx.theme;
    let grid = egui::Grid::new("keys")
        .num_columns(2)
        .min_col_width(COLUMN)
        .min_row_height(0.0)
        .spacing([GAP, theme.tokens.metrics.space.sm]);
    grid.show(ui, |ui| {
        for (category, title) in CATEGORIES {
            ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
            ui.end_row();
            for (command, chords) in bound(cx, category) {
                ui.label(command);
                widgets::kbd_row(ui, theme, &chords);
                ui.end_row();
            }
        }
    });
}

/// The narrow sheet: each category's commands with their key caps on the line below.
fn key_list(ui: &mut egui::Ui, cx: &FrameCx<'_>) {
    let theme = &cx.theme;
    let m = &theme.tokens.metrics;
    for (index, (category, title)) in CATEGORIES.into_iter().enumerate() {
        if index > 0 {
            ui.add_space(m.space.md);
        }
        ui.label(RichText::new(title).font(theme.fonts.body_strong.clone()));
        for (command, chords) in bound(cx, category) {
            ui.add_space(m.space.xxs);
            ui.label(command);
            widgets::kbd_row(ui, theme, &chords);
        }
    }
}

/// Each bound command in `category`: its title and every chord.
fn bound(cx: &FrameCx<'_>, category: Category) -> Vec<(&'static str, Vec<String>)> {
    COMMANDS
        .iter()
        .filter(|c| c.category == category)
        .map(|spec| {
            let chords = cx
                .keymap
                .bindings()
                .iter()
                .filter(|b| b.command == spec.id)
                .map(|b| b.chord.to_string())
                .collect();
            (spec.title, chords)
        })
        .filter(|(_, chords): &(_, Vec<String>)| !chords.is_empty())
        .collect()
}
