//! The pull request table: virtualized rows, sortable columns resized from the header, group
//! headers, the change bar in the margin, id-keyed selection, and a context menu per row.

mod cells;
mod header;

use eframe::egui::{self, Align, Layout, Rect, Sense, Stroke, StrokeKind};
use egui_extras::{Column, TableBody, TableBuilder, TableRow};
use prm_domain::view::{PrRow, age::AgeCache};

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    render::hover_bands::TableGeometry,
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    view::{
        DashboardView, ScrollIntent,
        columns::{
            self, ColumnCheck, ColumnLayout, ColumnSpec, Room, Visibility, Width, columns_hash,
            visible_columns,
        },
        display::DisplayRow,
    },
    widgets::controls::Check,
};
use header::{HeaderCx, header_row};

pub(crate) struct TableInputs<'a> {
    pub ages: &'a AgeCache,
    pub show_org: bool,
    /// The window has focus (draws the keyboard focus ring).
    pub focused: bool,
}

/// What the table measured this frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TableOut {
    pub geometry: Option<TableGeometry>,
}

/// `saved` is the user's dragged width for resizable columns.
fn column(spec: &ColumnSpec, saved: Option<f32>, margin: f32) -> Column {
    let column = match spec.width {
        Width::Exact => Column::exact(margin),
        Width::Fixed(width) => Column::exact(width),
        Width::Remainder => Column::remainder().at_least(spec.min).clip(true),
        Width::Initial(width) => Column::initial(saved.unwrap_or(width))
            .at_least(spec.min)
            .clip(true),
    };
    // why: egui_extras' resize handles run the full table height, so they showed a grabber
    // over group headers (the margin's sat on the chevron) and took clicks from rows. Widths
    // change from header-only strips instead (`header::resize_strips`).
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
    let columns = table_columns(ui, &theme, view, input.show_org);
    let span = ui.available_rect_before_wrap().x_range();
    let builder = table_builder(ui, &theme, &columns, view);
    let mut pending = Vec::new();
    let mut resize = None;
    let row_height = metrics.row_height;
    let output = builder
        .header(metrics.header_height, |mut header| {
            let hcx = HeaderCx {
                theme: &theme,
                span,
                columns: &columns,
                view,
            };
            resize = header_row(&mut header, &hcx, &mut pending);
        })
        .body(|body| {
            let bcx = BodyCx {
                theme: &theme,
                columns: &columns,
                view,
                input,
                cx_now: cx,
            };
            table_body(body, &bcx, &mut pending);
        });
    for action in pending {
        cx.push(action);
    }
    if let Some(resize) = resize {
        view.set_column_width(resize.key, resize.width);
    }
    let offset = output.state.clamped_offset().y;
    view.offset_y = offset;
    view.top_visible = Some(first_visible(offset, row_height));
    let rect = output.inner_rect;
    let ppp = ui.ctx().pixels_per_point();
    TableOut {
        geometry: Some(TableGeometry::new(
            [rect.left(), rect.top(), rect.right(), rect.bottom()],
            offset,
            row_height,
            ppp,
        )),
    }
}

/// This frame's columns for the room the table has, noting the chosen ones it had to leave out.
fn table_columns(
    ui: &egui::Ui,
    theme: &UiTheme,
    view: &mut DashboardView,
    show_org: bool,
) -> Vec<ColumnSpec> {
    let spacing = ui.spacing();
    let room = Room {
        // why: egui_extras keeps the scroll bar's lane out of the columns' width.
        width: ui.available_width() - spacing.scroll.allocated_width(),
        gap: spacing.item_spacing.x,
        margin: theme.tokens.metrics.row.margin_col,
    };
    let layout = ColumnLayout {
        group: view.group,
        show_org,
        hidden: &view.hidden_columns,
        widths: &view.widths,
    };
    let visible = visible_columns(room, &layout);
    view.squeezed.clear();
    view.squeezed
        .extend(columns::squeezed(&visible, &view.hidden_columns));
    visible
}

/// A listed column's check in the column menu or the settings checklist; `add` draws it.
/// Status and Title are locked on; a column the table has no room for reads muted and says so.
pub(crate) fn column_choice(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    column: &ColumnSpec,
    check: ColumnCheck,
    add: impl FnOnce(&mut egui::Ui, Check, egui::WidgetText) -> egui::Response,
) -> egui::Response {
    let locked = column.visibility() == Visibility::Locked;
    let name = egui::RichText::new(column.name);
    let (state, label, hint) = match check {
        ColumnCheck::Off => (Check::Off, name, None),
        ColumnCheck::On => (Check::On, name, None),
        ColumnCheck::Squeezed => (
            Check::On,
            name.color(theme.colors.text_muted),
            Some(copy::COLUMN_SQUEEZED),
        ),
    };
    let response = ui
        .add_enabled_ui(!locked, |ui| add(ui, state, label.into()))
        .inner
        .on_disabled_hover_text(copy::COLUMN_LOCKED);
    match hint {
        Some(hint) => response.on_hover_text(hint),
        None => response,
    }
}

/// Columns with their saved widths, and any pending scroll request (taken from the view).
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
        builder = builder.column(column(spec, view.widths.get(spec.key).copied(), margin));
    }
    match std::mem::take(&mut view.scroll) {
        ScrollIntent::ToRow(row, align) => builder.scroll_to_row(row, align),
        ScrollIntent::Offset(offset) => builder.vertical_scroll_offset(offset),
        // why: a new id is a new scroll area, which starts at the top; showing, hiding or
        // squeezing a column must not move the rows.
        ScrollIntent::None if renamed => builder.vertical_scroll_offset(view.offset_y),
        ScrollIntent::None => builder,
    }
}

/// The display index of the first row under the top edge.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floored and clamped to zero first"
)]
fn first_visible(offset: f32, row_height: f32) -> usize {
    (offset / row_height.max(1.0)).floor().max(0.0) as usize
}

struct BodyCx<'a, 'b> {
    theme: &'a UiTheme,
    columns: &'a [ColumnSpec],
    view: &'a DashboardView,
    input: &'a TableInputs<'a>,
    cx_now: &'a FrameCx<'b>,
}

fn table_body(mut body: TableBody<'_>, bcx: &BodyCx<'_, '_>, pending: &mut Vec<UiAction>) {
    let full = body.max_rect();
    let clip = body.ui_mut().clip_rect();
    let height = bcx.theme.tokens.metrics.row.row_height;
    let total = bcx.view.display.rows.len();
    body.rows(height, total, |mut row| {
        let index = row.index();
        match bcx.view.display.rows.get(index) {
            Some(DisplayRow::Group(span)) => group_row(&mut row, bcx, *span, (full, clip), pending),
            Some(DisplayRow::Pr(_)) => {
                if let Some(pr) = bcx.view.row_at(index) {
                    pr_row(&mut row, bcx, pr, index, pending);
                }
            }
            None => {}
        }
    });
}

/// A group header: the whole row (chevron included) collapses and expands it.
fn group_row(
    row: &mut TableRow<'_, '_>,
    bcx: &BodyCx<'_, '_>,
    span: u32,
    (full, clip): (Rect, Rect),
    pending: &mut Vec<UiAction>,
) {
    let Some(group) = bcx.view.model.groups.get(span as usize) else {
        return;
    };
    let theme = bcx.theme;
    let collapsed = bcx
        .view
        .collapsed
        .contains(&crate::view::display::group_id(&group.key));
    // why: striped rows fill each cell as it is added, so paint from the last cell, on top.
    let last = bcx.columns.len().saturating_sub(1);
    for (i, _) in bcx.columns.iter().enumerate() {
        row.col(|ui| {
            if i == last {
                group_label(ui, theme, group, collapsed, (full, clip));
            }
        });
    }
    let response = row.response();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, !collapsed, group.label.as_ref())
    });
    if response.clicked() {
        pending.push(UiAction::ToggleGroup(span));
    }
}

/// Paints a group header across the whole row from inside one of its cells.
fn group_label(
    ui: &egui::Ui,
    theme: &UiTheme,
    group: &prm_domain::view::group::GroupSpan,
    collapsed: bool,
    (full, clip): (Rect, Rect),
) {
    let cell = ui.max_rect();
    let line = Rect::from_x_y_ranges(full.x_range(), cell.y_range()).intersect(clip);
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(line);
    let chevron = if collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    let y = line.center().y;
    let icon = Rect::from_center_size(egui::pos2(line.left() + 12.0, y), egui::vec2(9.0, 9.0));
    icons::paint(&painter, icon, chevron, theme.colors.text_muted);
    let label = painter.layout_no_wrap(
        group.label.to_string(),
        theme.fonts.body_strong.clone(),
        theme.colors.text_primary,
    );
    let label_w = label.size().x;
    let x = line.left() + 26.0;
    painter.galley(
        egui::pos2(x, y - label.size().y / 2.0),
        label,
        theme.colors.text_primary,
    );
    let count = painter.layout_no_wrap(
        group.len.to_string(),
        theme.fonts.small.clone(),
        theme.colors.text_muted,
    );
    let at = egui::pos2(x + label_w + 8.0, y - count.size().y / 2.0);
    painter.galley(at, count, theme.colors.text_muted);
    painter.hline(
        line.x_range(),
        line.bottom() - 0.5,
        Stroke::new(
            theme.tokens.metrics.stroke.hairline,
            theme.colors.border_subtle,
        ),
    );
}

fn pr_row(
    row: &mut TableRow<'_, '_>,
    bcx: &BodyCx<'_, '_>,
    pr: &PrRow,
    index: usize,
    pending: &mut Vec<UiAction>,
) {
    let selected = bcx.view.selected.as_ref() == Some(&pr.key);
    row.set_selected(selected);
    for spec in bcx.columns {
        row.col(|ui| cells::cell(ui, bcx, spec, pr, index));
    }
    let response = row.response();
    if selected && bcx.view.keyboard_nav && bcx.input.focused {
        let rect = response.rect.shrink(1.0);
        let stroke = Stroke::new(
            bcx.theme.tokens.metrics.stroke.focus,
            bcx.theme.colors.border_focus,
        );
        response.ctx.layer_painter(response.layer_id).rect_stroke(
            rect,
            2.0,
            stroke,
            StrokeKind::Inside,
        );
    }
    if response.clicked() || response.secondary_clicked() {
        pending.push(UiAction::Select(pr.key.clone()));
    }
    if response.double_clicked() {
        pending.push(UiAction::Open(pr.key.clone()));
    }
    response.context_menu(|ui| context_menu(ui, pr, pending));
}

fn context_menu(ui: &mut egui::Ui, pr: &PrRow, pending: &mut Vec<UiAction>) {
    let items = [
        (copy::OPEN_IN_BROWSER, CommandId::PrOpen),
        (copy::COPY_LINK, CommandId::PrCopyLink),
        (copy::COPY_MARKDOWN, CommandId::PrCopyMarkdown),
        (copy::COPY_TITLE, CommandId::PrCopyTitle),
        (copy::PEEK, CommandId::PrPeek),
        (copy::MARK_SEEN, CommandId::PrMarkSeen),
    ];
    for (label, command) in items {
        if command == CommandId::PrMarkSeen && !pr.unread {
            continue;
        }
        if ui.button(label).clicked() {
            pending.push(UiAction::Select(pr.key.clone()));
            pending.push(UiAction::Run(command));
            ui.close();
        }
    }
}
