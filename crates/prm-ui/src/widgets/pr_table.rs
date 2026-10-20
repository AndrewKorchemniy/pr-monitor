//! The pull request table: virtualized rows, resizable sortable columns, group headers, the
//! change bar in the margin, id-keyed selection, and a context menu per row.

use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Stroke, StrokeKind};
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
        columns::{ColumnId, ColumnSpec, Width, columns_hash, visible_columns},
        display::DisplayRow,
    },
    widgets::{status_pill::status_pill, vote_tally::vote_tally},
};

/// Below this Status column width only the glyph shows.
const COMPACT_STATUS: f32 = 100.0;

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

fn column(spec: &ColumnSpec, saved: Option<f32>, margin: f32) -> Column {
    match spec.width {
        Width::Exact => Column::exact(margin),
        Width::Remainder => Column::remainder().at_least(spec.min).clip(true),
        Width::Initial(width) => Column::initial(saved.unwrap_or(width))
            .at_least(spec.min)
            .clip(true)
            .resizable(true),
    }
}

pub(crate) fn pr_table(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    view: &mut DashboardView,
    input: &TableInputs<'_>,
) -> TableOut {
    let theme = cx.theme.clone();
    let metrics = &theme.tokens.metrics.row;
    let columns = visible_columns(ui.available_width(), input.show_org, &view.hidden_columns);
    let builder = table_builder(ui, &theme, &columns, view);
    let mut pending = Vec::new();
    let mut widths: Vec<f32> = Vec::new();
    let row_height = metrics.row_height;
    let output = builder
        .header(metrics.header_height, |mut header| {
            header_row(&mut header, &theme, &columns, view, &mut pending);
        })
        .body(|body| {
            widths = body.widths().to_vec();
            table_body(
                body,
                &BodyCx {
                    theme: &theme,
                    columns: &columns,
                    view,
                    input,
                    cx_now: cx,
                },
                &mut pending,
            );
        });
    for action in pending {
        cx.push(action);
    }
    remember_widths(view, &columns, &widths);
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

/// Columns with their saved widths, and any pending scroll request (taken from the view).
fn table_builder<'a>(
    ui: &'a mut egui::Ui,
    theme: &UiTheme,
    columns: &[ColumnSpec],
    view: &mut DashboardView,
) -> TableBuilder<'a> {
    let margin = theme.tokens.metrics.row.margin_col;
    let mut builder = TableBuilder::new(ui)
        .id_salt(("pr_table", columns_hash(columns), view.table_epoch))
        .striped(theme.tokens.metrics.stripes)
        .sense(Sense::click())
        .resizable(true)
        .auto_shrink(false)
        .animate_scrolling(theme.policy.animations)
        .cell_layout(Layout::left_to_right(Align::Center));
    for spec in columns {
        builder = builder.column(column(spec, view.widths.get(spec.key).copied(), margin));
    }
    match std::mem::take(&mut view.scroll) {
        ScrollIntent::ToRow(row, align) => builder.scroll_to_row(row, align),
        ScrollIntent::Offset(offset) => builder.vertical_scroll_offset(offset),
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

/// Keeps user-resized widths of resizable columns (persisted with the UI state).
fn remember_widths(view: &mut DashboardView, columns: &[ColumnSpec], widths: &[f32]) {
    for (spec, width) in columns.iter().zip(widths) {
        if matches!(spec.width, Width::Initial(_)) {
            let rounded = width.round();
            if view.widths.get(spec.key) != Some(&rounded) {
                view.widths.insert(spec.key.to_owned(), rounded);
            }
        }
    }
}

fn header_row(
    header: &mut TableRow<'_, '_>,
    theme: &UiTheme,
    columns: &[ColumnSpec],
    view: &DashboardView,
    pending: &mut Vec<UiAction>,
) {
    for spec in columns {
        let mut clicked = false;
        header.col(|ui| clicked = header_cell(ui, theme, spec, view));
        if clicked && let Some(key) = spec.sort {
            pending.push(UiAction::Sort(key));
        }
    }
}

/// The column title with a sort arrow on the sorted column; true when clicked.
fn header_cell(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    spec: &ColumnSpec,
    view: &DashboardView,
) -> bool {
    let sorted = spec.sort.is_some_and(|key| key == view.sort.key);
    let color = if sorted {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };
    let label = egui::Label::new(
        RichText::new(spec.header)
            .font(theme.fonts.label.clone())
            .color(color),
    )
    .sense(if spec.sort.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    })
    .truncate();
    let layout = if spec.align_right {
        Layout::right_to_left(Align::Center)
    } else {
        Layout::left_to_right(Align::Center)
    };
    let arrow = if view.sort.descending {
        Icon::ChevronDown
    } else {
        Icon::ChevronUp
    };
    ui.with_layout(layout, |ui| {
        let clicked = ui.add(label).clicked();
        if sorted {
            icons::show(ui, arrow, 8.0, theme.colors.text_secondary);
        }
        clicked
    })
    .inner
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
    for (i, _) in bcx.columns.iter().enumerate() {
        row.col(|ui| {
            if i != 0 {
                return;
            }
            let cell = ui.max_rect();
            let line = Rect::from_x_y_ranges(full.x_range(), cell.y_range()).intersect(clip);
            let painter = ui.painter().with_clip_rect(line);
            let chevron = if collapsed {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            };
            let icon = Rect::from_center_size(
                egui::pos2(line.left() + 10.0, line.center().y),
                egui::vec2(10.0, 10.0),
            );
            icons::paint(&painter, icon, chevron, theme.colors.text_muted);
            let text = format!("{}  {}", group.label, group.len);
            let galley = painter.layout_no_wrap(
                text,
                theme.fonts.body_strong.clone(),
                theme.colors.text_primary,
            );
            painter.galley(
                egui::pos2(line.left() + 22.0, line.center().y - galley.size().y / 2.0),
                galley,
                theme.colors.text_primary,
            );
            let rule_y = line.center().y;
            painter.hline(
                (line.left() + 160.0)..=line.right() - 8.0,
                rule_y,
                Stroke::new(1.0, theme.colors.border_subtle),
            );
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
        row.col(|ui| cell(ui, bcx, spec, pr, index));
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

fn cell(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, spec: &ColumnSpec, pr: &PrRow, index: usize) {
    let theme = bcx.theme;
    let text = |s: &str| {
        RichText::new(s)
            .font(theme.fonts.body.clone())
            .color(theme.colors.text_primary)
    };
    let mono = |s: &str| {
        RichText::new(s)
            .font(theme.fonts.mono.clone())
            .color(theme.colors.text_secondary)
    };
    match spec.id {
        ColumnId::Margin => change_bar(ui, bcx, pr),
        ColumnId::Status => {
            let compact = ui.available_width() < COMPACT_STATUS;
            status_pill(ui, theme, pr.status, compact).on_hover_text(copy::status_tooltip(
                pr.status,
                pr.tally,
                pr.active_threads,
            ));
        }
        ColumnId::Number => right(ui, |ui| {
            ui.label(mono(&pr.number_text));
        }),
        ColumnId::Title => title_cell(ui, bcx, pr),
        ColumnId::Repo => {
            ui.add(egui::Label::new(text(&pr.repo_name)).truncate());
        }
        ColumnId::Author => {
            ui.add(egui::Label::new(text(&pr.author.display_name())).truncate());
        }
        ColumnId::Reviews => {
            vote_tally(ui, theme, &pr.reviewers);
        }
        ColumnId::Comments => right(ui, |ui| {
            let color = if pr.active_threads > 0 {
                theme.colors.tone(crate::theme::Tone::ActiveComments).fg
            } else {
                theme.colors.text_muted
            };
            ui.label(
                RichText::new(pr.comments_text.as_ref())
                    .font(theme.fonts.mono.clone())
                    .color(color),
            );
        }),
        ColumnId::Age => right(ui, |ui| {
            let age = bcx
                .input
                .ages
                .get(bcx.view.display.pr_at(index).unwrap_or(usize::MAX))
                .unwrap_or(&pr.age);
            let when = crate::format::date_time(pr.created_at, bcx.cx_now.offset);
            ui.label(mono(&age.text)).on_hover_text(when);
        }),
        ColumnId::Org => {
            ui.add(egui::Label::new(text(&pr.org_label)).truncate());
        }
    }
}

fn right(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

fn title_cell(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, pr: &PrRow) {
    let theme = bcx.theme;
    if pr.draft {
        ui.label(
            RichText::new(copy::DRAFT)
                .font(theme.fonts.small.clone())
                .color(theme.colors.text_muted),
        );
    }
    if pr.conflicts {
        icons::show(
            ui,
            Icon::Warning,
            11.0,
            theme.colors.tone(crate::theme::Tone::Warning).solid,
        )
        .on_hover_text(copy::CONFLICTS);
    }
    let color = if pr.stale {
        theme.colors.text_muted
    } else {
        theme.colors.text_primary
    };
    let label = egui::Label::new(
        RichText::new(pr.title.as_ref())
            .font(theme.fonts.body.clone())
            .color(color),
    )
    .truncate();
    let response = ui.add(label);
    if let Some(note) = bcx.cx_now.changes.note(&pr.key) {
        response.on_hover_text(note.text.as_ref());
    } else if pr.stale {
        let when = pr.last_activity;
        response.on_hover_text(copy::stale_row(&crate::format::clock(
            when,
            bcx.cx_now.offset,
        )));
    }
}

/// The 3 px blue-pencil bar for rows changed since last looked; draws downward when it appears.
fn change_bar(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, pr: &PrRow) {
    let theme = bcx.theme;
    let id = ui.scope_id().with(("bar", &pr.key));
    let motion = &theme.tokens.metrics.motion;
    let duration = if theme.policy.animations {
        motion.reveal
    } else {
        0.0
    };
    let t = ui.ctx().animate_bool_with_time(id, pr.unread, duration);
    if t <= 0.0 {
        return;
    }
    let cell = ui.max_rect();
    let width = theme.tokens.metrics.row.revision_bar;
    let top = cell.top() + 2.0;
    let bottom = top + (cell.height() - 4.0) * t;
    let bar = Rect::from_min_max(
        egui::pos2(cell.left() + 1.0, top),
        egui::pos2(cell.left() + 1.0 + width, bottom),
    );
    ui.painter()
        .rect_filled(bar, 1.0, theme.colors.signature_revision);
    let _ = ui
        .allocate_rect(bar, Sense::hover())
        .on_hover_text(copy::CHANGED_TOOLTIP);
}
