//! What the table and the card list share row by row: the collapsible group header, how a pull
//! request row answers the pointer (select, open, context menu, focus ring), the pending scroll
//! request, and what the list records about where it scrolled.

use eframe::egui::{self, Rect, Response, Stroke, StrokeKind};
use egui_extras::{TableBuilder, TableRow};
use prm_domain::view::{GroupBy, PrRow, group::GroupSpan};

use super::TableInputs;
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
    view::{DashboardView, ScrollIntent, display::Pitch},
};

/// What a row's painters read: the theme, the view, the list's inputs and the frame.
pub(crate) struct RowCx<'a, 'b> {
    pub theme: &'a UiTheme,
    pub view: &'a DashboardView,
    pub input: &'a TableInputs<'a>,
    pub frame: &'a FrameCx<'b>,
}

/// A group header: the whole row (chevron included) collapses and expands it. `cells` is how
/// many cells the row has; `(full, clip)` are the list's rect and clip.
pub(crate) fn group_row(
    row: &mut TableRow<'_, '_>,
    rcx: &RowCx<'_, '_>,
    (span, cells): (u32, usize),
    (full, clip): (Rect, Rect),
    pending: &mut Vec<UiAction>,
) {
    let Some(group) = rcx.view.model.groups.get(span as usize) else {
        return;
    };
    let collapsed = rcx
        .view
        .collapsed
        .contains(&crate::view::display::group_id(&group.key));
    // why: striped rows fill each cell as it is added, so paint from the last cell, on top.
    let last = cells.saturating_sub(1);
    for i in 0..cells {
        row.col(|ui| {
            if i == last {
                group_label(ui, rcx.theme, group, collapsed, (full, clip));
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
    group: &GroupSpan,
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

/// Whether `pr` is the selected row; marks the table row so it fills like the others.
pub(crate) fn mark_selected(row: &mut TableRow<'_, '_>, rcx: &RowCx<'_, '_>, pr: &PrRow) -> bool {
    let selected = rcx.view.selected.as_ref() == Some(&pr.key);
    row.set_selected(selected);
    selected
}

/// What a pull request row does with its response: the keyboard focus ring while selected, a
/// click selects, a double-click opens, and a right-click selects and offers the context menu.
pub(crate) fn answer_row(
    response: &Response,
    rcx: &RowCx<'_, '_>,
    (pr, selected): (&PrRow, bool),
    pending: &mut Vec<UiAction>,
) {
    if selected && rcx.view.keyboard_nav && rcx.input.focused {
        let rect = response.rect.shrink(1.0);
        let stroke = Stroke::new(
            rcx.theme.tokens.metrics.stroke.focus,
            rcx.theme.colors.border_focus,
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

/// The view's pending scroll request, taken from it. A list whose id changed (`renamed`) is a
/// new scroll area, which starts at the top, so it gets the view's offset back.
pub(crate) fn scroll_request<'a>(
    builder: TableBuilder<'a>,
    view: &mut DashboardView,
    renamed: bool,
) -> TableBuilder<'a> {
    match std::mem::take(&mut view.scroll) {
        ScrollIntent::ToRow(row, align) => builder.scroll_to_row(row, align),
        ScrollIntent::Offset(offset) => builder.vertical_scroll_offset(offset),
        ScrollIntent::None if renamed => builder.vertical_scroll_offset(view.offset_y),
        ScrollIntent::None => builder,
    }
}

/// Where a list drawn at `pitch` scrolled to: the view keeps the offset, the top row and the
/// pitch for anchoring; the hover bands get the list's rows, when they are evenly spaced.
pub(crate) fn record_scroll(
    ui: &egui::Ui,
    view: &mut DashboardView,
    (inner, offset): (Rect, f32),
    pitch: Pitch,
) -> Option<TableGeometry> {
    view.offset_y = offset;
    view.pitch = pitch;
    view.top_visible = Some(pitch.index_at(&view.display.rows, offset));
    // why: the hover bands assume one pitch; cards between shorter group headers have two, and
    // a wrong band would hold a stale hover until the next band.
    let even = pitch.is_uniform() || view.group == GroupBy::None;
    even.then(|| {
        TableGeometry::new(
            [inner.left(), inner.top(), inner.right(), inner.bottom()],
            offset,
            pitch.pr,
            ui.ctx().pixels_per_point(),
        )
    })
}
