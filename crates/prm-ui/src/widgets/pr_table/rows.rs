//! What the table and the card list share row by row: the collapsible group header, how a pull
//! request row answers the pointer (select, open, context menu, focus ring), the pending scroll
//! request, and what the list records about where it scrolled.

use eframe::egui::{self, Rect, Response, Stroke, StrokeKind};
use egui_extras::{TableBody, TableBuilder, TableRow};
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
    widgets::controls,
};

/// What a row's painters read: the theme, the view, the list's inputs and the frame.
pub(crate) struct RowCx<'a, 'b> {
    pub theme: &'a UiTheme,
    pub view: &'a DashboardView,
    pub input: &'a TableInputs<'a>,
    pub frame: &'a FrameCx<'b>,
    /// How far a group header's label starts from the list's left edge, so it lines up with
    /// the text of the rows below; its chevron sits just before it.
    pub label_x: f32,
}

/// Where a table's group labels start (points from the list's left edge).
pub(crate) const TABLE_LABEL_X: f32 = 26.0;
/// From a group label back to the centre of its chevron (points).
const CHEVRON_BACK: f32 = 14.0;

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
                group_label(ui, rcx, (group, collapsed), (full, clip));
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
    rcx: &RowCx<'_, '_>,
    (group, collapsed): (&GroupSpan, bool),
    (full, clip): (Rect, Rect),
) {
    let theme = rcx.theme;
    let cell = ui.max_rect();
    let line = Rect::from_x_y_ranges(full.x_range(), cell.y_range()).intersect(clip);
    // why: the row's band reaches half a gap past the cell; a header owns all of it, on the
    // canvas rather than a stripe, with its rule on the band's bottom edge.
    let half = ui.spacing().item_spacing.y / 2.0;
    let band = line.expand2(egui::vec2(0.0, half)).intersect(clip);
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(band);
    let fill = if ui.rect_contains_pointer(band) {
        theme.colors.row_hover
    } else {
        theme.colors.surface_canvas
    };
    painter.rect_filled(band, 0.0, fill);
    let chevron = if collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    let y = line.center().y;
    let x = line.left() + rcx.label_x;
    let icon = Rect::from_center_size(egui::pos2(x - CHEVRON_BACK, y), egui::vec2(9.0, 9.0));
    icons::paint(&painter, icon, chevron, theme.colors.text_muted);
    let label = painter.layout_no_wrap(
        group.label.to_string(),
        theme.fonts.body_strong.clone(),
        theme.colors.text_primary,
    );
    let label_w = label.size().x;
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
        band.bottom() - 0.5,
        Stroke::new(
            theme.tokens.metrics.stroke.hairline,
            theme.colors.border_subtle,
        ),
    );
}

/// Where a list's rows lie: the body's rect as far as a row's cells and the half gap after the
/// last one reach, short of the scroll bar's lane when the columns leave it.
pub(crate) fn rows_rect(body: &mut TableBody<'_>) -> Rect {
    let full = body.max_rect();
    let gap = body.ui_mut().spacing().item_spacing.x;
    let widths = body.widths();
    let reach = widths.iter().sum::<f32>() + gap * (widths.len() as f32 - 0.5);
    full.with_max_x((full.left() + reach).min(full.right()))
}

/// Whether `pr` is the selected row; marks the table row so it fills like the others.
pub(crate) fn mark_selected(row: &mut TableRow<'_, '_>, rcx: &RowCx<'_, '_>, pr: &PrRow) -> bool {
    let selected = rcx.view.selected.as_ref() == Some(&pr.key);
    row.set_selected(selected);
    selected
}

/// What a pull request row does with its response: the keyboard focus ring while selected, a
/// click selects, a double-click opens, and a right-click selects and offers the context menu.
/// `list` is where the list's rows lie (`rows_rect`).
pub(crate) fn answer_row(
    response: &Response,
    rcx: &RowCx<'_, '_>,
    (pr, selected): (&PrRow, bool),
    list: Rect,
    pending: &mut Vec<UiAction>,
) {
    if selected && rcx.view.keyboard_nav && rcx.input.focused {
        focus_ring(response, rcx.theme, list);
    }
    if response.clicked() || response.secondary_clicked() {
        pending.push(UiAction::Select(pr.key.clone()));
    }
    if response.double_clicked() {
        pending.push(UiAction::Open(pr.key.clone()));
    }
    // why: rows are many and a menu is open on one at most; build it only for that row.
    let menu = egui::Popup::default_response_id(response);
    if !response.secondary_clicked() && !egui::Popup::is_id_open(&response.ctx, menu) {
        return;
    }
    egui::Popup::context_menu(response)
        .frame(controls::menu_frame(rcx.theme))
        .width(controls::MENU_WIDTH)
        .show(|ui| {
            ui.set_width(controls::MENU_WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            context_menu(ui, rcx.theme, pr, pending);
        });
}

/// The ring round the selected row: an even inset inside the row's selection fill, starting
/// after the change bar, cut to the list's width and clipped to the list.
fn focus_ring(response: &Response, theme: &UiTheme, list: Rect) {
    // why: a row's response takes in what its cells' contents allocate, which can reach past
    // the list's right edge; there the ring ran off the window and had no right side.
    let x = response.rect.x_range().intersection(list.x_range());
    // why: the fill reaches half a gap past the cells on each side; inset from that, not the
    // cells, so the ring keeps clear of the text and the bar.
    let gap = response.ctx.global_style().spacing.item_spacing;
    let fill = Rect::from_x_y_ranges(x, response.rect.y_range()).expand2(gap * 0.5);
    let inset = gap.y * 0.5;
    let after_bar = list.left() + 1.0 + theme.tokens.metrics.row.revision_bar + inset;
    let ring = fill
        .intersect(list.expand2(egui::vec2(0.0, gap.y * 0.5)))
        .shrink(inset)
        .with_min_x(after_bar);
    let stroke = Stroke::new(theme.tokens.metrics.stroke.focus, theme.colors.border_focus);
    response
        .ctx
        .layer_painter(response.layer_id)
        .with_clip_rect(list)
        .rect_stroke(ring, 2.0, stroke, StrokeKind::Inside);
}

/// The row's actions as menu rows, grouped: open, the copies, then peek and mark as seen.
fn context_menu(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow, pending: &mut Vec<UiAction>) {
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
        if controls::menu_item(ui, theme, label, false).clicked() {
            pending.push(UiAction::Select(pr.key.clone()));
            pending.push(UiAction::Run(command));
            ui.close();
        }
        if matches!(command, CommandId::PrOpen | CommandId::PrCopyTitle) {
            controls::spaced_divider(ui, theme, theme.tokens.metrics.space.xs);
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
/// pitch for anchoring, and how far a page moves; the hover bands get the list's rows, when
/// they are evenly spaced.
pub(crate) fn record_scroll(
    ui: &egui::Ui,
    view: &mut DashboardView,
    (inner, offset): (Rect, f32),
    pitch: Pitch,
) -> Option<TableGeometry> {
    view.offset_y = offset;
    view.pitch = pitch;
    view.top_visible = Some(pitch.index_at(&view.display.rows, offset));
    let whole = pitch.whole_prs(&view.display.rows, (offset, inner.height()));
    view.page_rows = whole.saturating_sub(1).max(1);
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
