//! The pull request list as cards, for a window too narrow for the table: one card per pull
//! request under the same group headers. It is the table's machinery with two columns (the
//! change-bar margin and the card), so it virtualizes, stripes, selects, scrolls to the
//! selection and answers the pointer exactly like the table (`pr_table::rows`).

mod card;

use eframe::egui::{self, Align, Layout, Sense};
use egui_extras::{Column, TableBody, TableBuilder, TableRow};

use crate::{
    actions::UiAction,
    frame::FrameCx,
    theme::UiTheme,
    view::{
        DashboardView, columns,
        display::{DisplayRow, Pitch},
    },
    widgets::pr_table::{
        TableInputs, TableOut, cells,
        rows::{
            self, RowCx, answer_row, group_row, mark_selected, record_scroll, rows_rect,
            scroll_request,
        },
    },
};

/// The margin and the card.
const CELLS: usize = 2;

/// How far apart the cards and the group headers between them are laid out (points).
pub(crate) fn card_pitch(ui: &egui::Ui, theme: &UiTheme) -> Pitch {
    let spacing = ui.spacing().item_spacing.y;
    Pitch {
        pr: card::height(ui, theme) + spacing,
        group: theme.tokens.metrics.row.row_height + spacing,
    }
}

pub(crate) fn pr_cards(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    view: &mut DashboardView,
    input: &TableInputs<'_>,
) -> TableOut {
    let theme = cx.theme.clone();
    // why: no table is shown, so the column checklist reads every chosen column as left out.
    view.squeezed.clear();
    view.squeezed
        .extend(columns::squeezed(&[], &view.hidden_columns));
    let pitch = card_pitch(ui, &theme);
    let spacing = ui.spacing().item_spacing.y;
    let spacing_x = ui.spacing().item_spacing.x;
    let heights = (pitch.pr - spacing, pitch.group - spacing);
    let builder = TableBuilder::new(ui)
        .id_salt("pr_cards")
        .striped(theme.tokens.metrics.stripes)
        .sense(Sense::click())
        .auto_shrink(false)
        .animate_scrolling(theme.policy.animations)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(theme.tokens.metrics.row.margin_col).resizable(false))
        .column(Column::remainder().clip(true).resizable(false));
    let builder = scroll_request(builder, view, false);
    let mut pending = Vec::new();
    let output = builder.body(|body| {
        let rcx = RowCx {
            theme: &theme,
            view,
            input,
            frame: cx,
            label_x: card::text_x(&theme, spacing_x),
        };
        cards_body(body, &rcx, heights, &mut pending);
    });
    for action in pending {
        cx.push(action);
    }
    let scrolled = (output.inner_rect, output.state.clamped_offset().y);
    TableOut {
        geometry: record_scroll(ui, view, scrolled, pitch),
    }
}

/// Cards and group headers, `(card, header)` points tall; only the visible ones are built.
fn cards_body(
    mut body: TableBody<'_>,
    rcx: &RowCx<'_, '_>,
    (card, header): (f32, f32),
    pending: &mut Vec<UiAction>,
) {
    let full = body.max_rect();
    let clip = body.ui_mut().clip_rect();
    let list = rows_rect(&mut body);
    let rows = &rcx.view.display.rows;
    let heights = rows.iter().map(|row| match row {
        DisplayRow::Group(_) => header,
        DisplayRow::Pr(_) => card,
    });
    body.heterogeneous_rows(heights, |mut row| match rows.get(row.index()) {
        Some(DisplayRow::Group(span)) => {
            group_row(&mut row, rcx, (*span, CELLS), (full, clip), pending);
        }
        Some(DisplayRow::Pr(_)) => card_row(&mut row, rcx, list, pending),
        None => {}
    });
}

/// A pull request's card; the cards lie in `list`.
fn card_row(
    row: &mut TableRow<'_, '_>,
    rcx: &RowCx<'_, '_>,
    list: egui::Rect,
    pending: &mut Vec<UiAction>,
) {
    let index = row.index();
    let Some(pr) = rcx.view.row_at(index) else {
        return;
    };
    let selected = mark_selected(row, rcx, pr);
    row.col(|ui| cells::change_bar(ui, rcx, pr));
    row.col(|ui| rows::last_cell(ui, rcx.theme, |ui| card::card(ui, rcx, pr, index)));
    answer_row(&row.response(), rcx, (pr, selected), list, pending);
}

#[cfg(test)]
mod tests;
