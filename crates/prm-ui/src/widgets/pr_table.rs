//! The pull request table: virtualized rows, sortable columns resized from the header, group
//! headers, the change bar in the margin, id-keyed selection, and a context menu per row. Its
//! cells and row behaviour are shared with the card list (`cells`, `rows`).

pub(crate) mod cells;
mod header;
mod reorder;
pub(crate) mod rows;

use eframe::egui::{self, Align, Layout, Sense};
use egui_extras::{Column, TableBody, TableBuilder, TableRow};
use prm_domain::view::{PrRow, age::AgeCache};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    render::hover_bands::TableGeometry,
    theme::UiTheme,
    view::{
        DashboardView,
        columns::{
            self, COLUMNS, ColumnCheck, ColumnLayout, ColumnSpec, Room, Visibility, Width,
            columns_hash, dragged_widths, visible_columns,
        },
        display::{DisplayRow, Pitch},
    },
    widgets::controls::{self, Check},
};
use header::{HeaderCx, HeaderOut, Reorder, Resize, header_row};
use reorder::{Slide, slide_in};
use rows::{RowCx, answer_row, group_row, mark_selected, record_scroll, rows_rect, scroll_request};

pub(crate) struct TableInputs<'a> {
    pub ages: &'a AgeCache,
    pub show_org: bool,
    /// The window has focus (draws the keyboard focus ring).
    pub focused: bool,
}

/// What the list measured this frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TableOut {
    pub geometry: Option<TableGeometry>,
}

/// The room the table's columns share in a list `width` points wide.
pub(crate) fn table_room(spacing: &egui::Spacing, theme: &UiTheme, width: f32) -> Room {
    Room {
        // why: egui_extras keeps the scroll bar's lane out of the columns' width, and the
        // trailing margin column (and the gap before it) is not a column the specs share.
        width: width
            - spacing.scroll.allocated_width()
            - theme.tokens.metrics.row.margin_col
            - spacing.item_spacing.x,
        gap: spacing.item_spacing.x,
        margin: theme.tokens.metrics.row.margin_col,
    }
}

/// How far apart the table lays out its rows: `egui_extras` adds the vertical item spacing to
/// each row's height.
pub(crate) fn table_pitch(spacing: &egui::Spacing, theme: &UiTheme) -> Pitch {
    Pitch::uniform(theme.tokens.metrics.row.row_height + spacing.item_spacing.y)
}

/// How far below the top of the list the table's rows start: its header and the spacing under
/// it (points).
pub(crate) fn header_pitch(spacing: &egui::Spacing, theme: &UiTheme) -> f32 {
    theme.tokens.metrics.row.header_height + spacing.item_spacing.y
}

/// Every column is fixed for `egui_extras`: Title takes the remainder each frame and the others
/// the width `visible_columns` gave them.
fn column(spec: &ColumnSpec, margin: f32) -> Column {
    let column = match spec.width {
        Width::Exact => Column::exact(margin),
        Width::Fixed(width) => Column::exact(width),
        Width::Remainder => Column::remainder().at_least(spec.min).clip(true),
        Width::Initial(width) => Column::initial(width).at_least(spec.min).clip(true),
    };
    // why: a resizable column keeps the width it had in `egui_extras`' state, so Title, the
    // remainder, stopped following the window. Its resize handles also ran the full table
    // height, over group headers and row clicks. Widths change from header-only strips instead
    // (`header::resize_strips`).
    column.resizable(false)
}

pub(crate) fn pr_table(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    view: &mut DashboardView,
    input: &TableInputs<'_>,
) -> TableOut {
    let theme = cx.theme.clone();
    let metrics = &theme.tokens.metrics.row;
    let (columns, room) = table_columns(ui, &theme, view, input.show_org);
    let span = ui.available_rect_before_wrap().x_range();
    let pitch = table_pitch(ui.spacing(), &theme);
    let builder = table_builder(ui, &theme, &columns, view);
    let mut pending = Vec::new();
    let mut found = HeaderOut::default();
    let output = builder
        .header(metrics.header_height, |mut header| {
            let hcx = HeaderCx {
                theme: &theme,
                span,
                columns: &columns,
                view,
            };
            found = header_row(&mut header, &hcx, &mut pending);
        })
        .body(|body| {
            let rcx = RowCx {
                theme: &theme,
                view,
                input,
                frame: cx,
                label_x: rows::TABLE_LABEL_X,
            };
            table_body(body, &rcx, (&columns, &found.slide), &mut pending);
        });
    for action in pending {
        cx.push(action);
    }
    if let Some(resize) = &found.resize {
        resize_column(view, (room, input.show_org), resize);
    }
    let moves = Moves {
        event: found.reorder.take(),
        landing: found.landing,
        rows: output.inner_rect,
    };
    reorder_columns(ui, &theme, view, (&columns, moves));
    let scrolled = (output.inner_rect, output.state.clamped_offset().y);
    TableOut {
        geometry: record_scroll(ui, view, scrolled, pitch),
    }
}

/// What the header found about moving a column, and where the table's rows lie.
struct Moves {
    event: Option<Reorder>,
    /// Where the lifted header will land.
    landing: Option<egui::Rangef>,
    rows: egui::Rect,
}

/// Does what the header asked for about moving a column: picks one up, follows it, drops it or
/// steps it a place, and washes the rows' column it will land in (`landing`).
fn reorder_columns(
    ui: &egui::Ui,
    theme: &UiTheme,
    view: &mut DashboardView,
    (columns, moves): (&[ColumnSpec], Moves),
) {
    let Moves {
        event,
        landing,
        rows,
    } = moves;
    let shown = |unit| columns.iter().any(|c| c.id.unit() == Some(unit));
    // why: a window resize can squeeze the lifted column out; there is nothing left to drop.
    if view.reorder.as_ref().is_some_and(|drag| !shown(drag.unit)) {
        view.cancel_column_drag();
    }
    match event {
        Some(Reorder::Lift { unit, grab }) => view.lift_column(unit, grab),
        Some(Reorder::Retarget { among }) => view.retarget_column(among, shown),
        Some(Reorder::Drop) => view.drop_column(),
        Some(Reorder::Step(unit, step)) => view.step_column((unit, step), shown),
        None => {}
    }
    if let Some(x) = landing {
        let rect = egui::Rect::from_x_y_ranges(x, rows.y_range());
        controls::wash(&ui.painter_at(rows), theme, rect);
    }
}

/// The user's column choices in the view.
fn column_layout(view: &DashboardView, show_org: bool) -> ColumnLayout<'_> {
    ColumnLayout {
        group: view.group,
        show_org,
        picks: view.picks(),
        widths: &view.widths,
        order: view.layout_order(),
    }
}

/// This frame's columns for the room the table has, noting the chosen ones it had to leave out.
fn table_columns(
    ui: &egui::Ui,
    theme: &UiTheme,
    view: &mut DashboardView,
    show_org: bool,
) -> (Vec<ColumnSpec>, Room) {
    let room = table_room(ui.spacing(), theme, ui.available_width());
    let visible = visible_columns(room, &column_layout(view, show_org));
    // Built from the fields, not `view.picks()`, so the squeezed buffer is refilled in place.
    let picks = columns::Picks {
        hidden: &view.hidden_columns,
        shown: &view.shown_columns,
    };
    view.squeezed.clear();
    view.squeezed.extend(columns::squeezed(&visible, picks));
    (visible, room)
}

/// Saves a width dragged in the header (`dragged_widths`), or forgets it on a double-click.
fn resize_column(view: &mut DashboardView, (room, show_org): (Room, bool), resize: &Resize) {
    let Some(width) = resize.width else {
        view.set_column_width(resize.key, None);
        return;
    };
    let saved = dragged_widths(room, &column_layout(view, show_org), (resize.key, width));
    for (key, width) in saved {
        view.set_column_width(key, Some(width));
    }
}

/// A listed column's check in the column menu or the settings checklist; `add` draws it.
/// Status and Title are locked on; a column the table has no room for says so on hover and,
/// with `note`, after its name.
pub(crate) fn column_choice(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (column, check, note): (&ColumnSpec, ColumnCheck, bool),
    add: impl FnOnce(&mut egui::Ui, Check, egui::WidgetText) -> egui::Response,
) -> egui::Response {
    let locked = column.visibility() == Visibility::Locked;
    let name = egui::WidgetText::from(column.name);
    let (state, label, hint) = match check {
        ColumnCheck::Off => (Check::Off, name, None),
        ColumnCheck::On => (Check::On, name, None),
        // why: a muted label read as locked; the name stays plain, with a short note after it
        // where there is room for one (`note`), and the reason on hover.
        ColumnCheck::Squeezed => (
            Check::On,
            if note {
                squeezed_label(theme, column.name)
            } else {
                name
            },
            Some(copy::COLUMN_SQUEEZED),
        ),
    };
    let response = ui
        .add_enabled_ui(!locked, |ui| add(ui, state, label))
        .inner
        .on_disabled_hover_text(copy::COLUMN_LOCKED);
    match hint {
        Some(hint) => response.on_hover_text(hint),
        None => response,
    }
}

/// "Name  No room": the column's name, then a muted note that the window is too narrow for it.
fn squeezed_label(theme: &UiTheme, name: &str) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    let c = &theme.colors;
    let format = |font: &egui::FontId, color| egui::TextFormat::simple(font.clone(), color);
    job.append(name, 0.0, format(&theme.fonts.body, c.text_primary));
    // why: the spaces are part of the text, so screen readers hear "Repository  No room".
    let note = format!("  {}", copy::COLUMN_NO_ROOM);
    job.append(&note, 0.0, format(&theme.fonts.small, c.text_muted));
    job.into()
}

/// Columns at this frame's widths, and any pending scroll request (taken from the view).
fn table_builder<'a>(
    ui: &'a mut egui::Ui,
    theme: &UiTheme,
    columns: &[ColumnSpec],
    view: &mut DashboardView,
) -> TableBuilder<'a> {
    let margin = theme.tokens.metrics.row.margin_col;
    let salt = (columns_hash(columns), view.table_epoch);
    let renamed = view
        .table_salt
        .replace(salt)
        .is_some_and(|last| last != salt);
    let mut builder = TableBuilder::new(ui)
        .id_salt(("pr_table", salt.0, salt.1))
        .striped(theme.tokens.metrics.stripes)
        .sense(Sense::click())
        .auto_shrink(false)
        .animate_scrolling(theme.policy.animations)
        .cell_layout(Layout::left_to_right(Align::Center));
    for spec in columns {
        builder = builder.column(column(spec, margin));
    }
    // why: a margin as wide as the first column's, so the fills reach the list's edge while
    // the last column's text keeps clear of it.
    builder = builder.column(Column::exact(margin).resizable(false));
    // why: showing, hiding or squeezing a column renames the table, and so its scroll area;
    // that must not move the rows.
    scroll_request(builder, view, renamed)
}

fn table_body(
    mut body: TableBody<'_>,
    rcx: &RowCx<'_, '_>,
    (columns, slide): (&[ColumnSpec], &Slide),
    pending: &mut Vec<UiAction>,
) {
    let full = body.max_rect();
    let clip = body.ui_mut().clip_rect();
    let list = rows_rect(&mut body);
    let height = rcx.theme.tokens.metrics.row.row_height;
    let total = rcx.view.display.rows.len();
    body.rows(height, total, |mut row| {
        let index = row.index();
        match rcx.view.display.rows.get(index) {
            Some(DisplayRow::Group(span)) => {
                group_row(
                    &mut row,
                    rcx,
                    (*span, columns.len() + 1),
                    (full, clip),
                    pending,
                );
            }
            Some(DisplayRow::Pr(_)) => {
                pr_row(&mut row, rcx, (columns, (slide, clip), list), pending);
            }
            None => {}
        }
    });
}

/// A pull request's row in the table; the table's rows lie in `list`.
fn pr_row(
    row: &mut TableRow<'_, '_>,
    rcx: &RowCx<'_, '_>,
    (columns, slide, list): (&[ColumnSpec], (&Slide, egui::Rect), egui::Rect),
    pending: &mut Vec<UiAction>,
) {
    let index = row.index();
    let Some(pr) = rcx.view.row_at(index) else {
        return;
    };
    let selected = mark_selected(row, rcx, pr);
    let mut slots = [egui::Rect::NOTHING; COLUMNS.len()];
    for (spec, slot) in columns.iter().zip(&mut slots) {
        row.col(|ui| {
            *slot = ui.max_rect();
            if !slide.0.sliding(spec.id) {
                cells::cell(ui, rcx, spec, pr, index);
            }
        });
    }
    // why: the trailing margin's cell comes last, so what slides is drawn over every fill.
    row.col(|ui| sliding_cells(ui, rcx, (columns, &slots, slide), (pr, index)));
    answer_row(&row.response(), rcx, (pr, selected), list, pending);
}

/// The cells of the columns that are sliding, each drawn where its slide puts it.
fn sliding_cells(
    ui: &mut egui::Ui,
    rcx: &RowCx<'_, '_>,
    (columns, slots, slide): (&[ColumnSpec], &[egui::Rect], (&Slide, egui::Rect)),
    (pr, index): (&PrRow, usize),
) {
    for (spec, slot) in columns.iter().zip(slots) {
        if slide.0.sliding(spec.id) {
            slide_in(ui, slide, (spec.id, *slot), |ui| {
                cells::cell(ui, rcx, spec, pr, index);
            });
        }
    }
}
