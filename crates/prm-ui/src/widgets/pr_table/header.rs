//! The table header. Each cell is one target: sortable columns fill on hover, preview the sort
//! arrow and sort on a single click; a right-click on any cell opens the column menu. Widths
//! change from drag strips on the dividers, which live in the header only.

use eframe::egui::{
    self, Color32, CursorIcon, Rect, Sense, Stroke, UiBuilder,
    text::{LayoutJob, TextWrapping},
};
use egui_extras::TableRow;
use prm_domain::view::Sort;

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    view::{
        DashboardView,
        columns::{self, COLUMNS, ColumnSpec, Visibility},
        next_sort,
    },
    widgets::controls,
};

/// The sort arrow.
const ARROW: f32 = 8.0;
/// Half a divider's drag strip, like the `egui_extras` resize grab.
const GRAB: f32 = 3.0;
/// The column menu.
const MENU_WIDTH: f32 = 200.0;

/// A width dragged this frame, or `None` when a double-click asked for the default back.
pub(super) struct Resize {
    pub key: &'static str,
    pub width: Option<f32>,
}

pub(super) struct HeaderCx<'a> {
    pub theme: &'a UiTheme,
    /// The table's horizontal extent.
    pub span: egui::Rangef,
    pub columns: &'a [ColumnSpec],
    pub view: &'a DashboardView,
}

pub(super) fn header_row(
    header: &mut TableRow<'_, '_>,
    hcx: &HeaderCx<'_>,
    pending: &mut Vec<UiAction>,
) -> Option<Resize> {
    let last = hcx.columns.len().saturating_sub(1);
    let mut cells = [Rect::NOTHING; COLUMNS.len()];
    let mut resize = None;
    for (index, spec) in hcx.columns.iter().enumerate() {
        header.col(|ui| {
            if index == 0 {
                header_rule(ui, hcx.theme, hcx.span);
            }
            header_col(ui, hcx, spec, pending);
            if index == last {
                // why: registered last, the strips sit on top of the cell targets they touch.
                resize = resize_strips(ui, hcx, cells.get(..index).unwrap_or_default());
            }
            if let Some(cell) = cells.get_mut(index) {
                *cell = ui.max_rect();
            }
        });
    }
    resize
}

/// A hairline under the whole header row, painted from its first cell.
fn header_rule(ui: &egui::Ui, theme: &UiTheme, span: egui::Rangef) {
    let cell = ui.max_rect();
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(Rect::from_x_y_ranges(span, cell.y_range().expand(4.0)));
    let y = cell.bottom() + ui.spacing().item_spacing.y / 2.0;
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_subtle,
    );
    painter.hline(span, y, stroke);
}

/// One header cell and what clicking it asks for.
fn header_col(
    ui: &mut egui::Ui,
    hcx: &HeaderCx<'_>,
    spec: &ColumnSpec,
    pending: &mut Vec<UiAction>,
) {
    let response = header_cell(ui, hcx.theme, spec, hcx.view.sort);
    if response.clicked()
        && let Some(key) = spec.sort
    {
        pending.push(UiAction::Sort(key));
    }
    column_menu(&response, hcx, pending);
}

/// The cell's target reaches the middle of the gaps either side, so the hover fill pads the
/// title and no click falls between cells.
fn header_cell(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    spec: &ColumnSpec,
    sort: Sort,
) -> egui::Response {
    let cell = ui.max_rect();
    let target = cell.expand2(egui::vec2(ui.spacing().item_spacing.x / 2.0, 0.0));
    let mut hit = ui.new_child(UiBuilder::new().max_rect(target));
    hit.set_clip_rect(target);
    let sense = if spec.sort.is_some() {
        Sense::click()
    } else {
        // why: the right-click menu needs clicks, but a cell that cannot sort is no Tab stop.
        Sense::click() - Sense::FOCUSABLE
    };
    let response = hit.interact(
        target,
        egui::Id::unique(("pr_table_header", spec.id)),
        sense,
    );
    let menu_open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));
    if spec.sort.is_some() || menu_open {
        controls::ghost_chrome(&hit, theme, target, &response, menu_open);
    }
    let look = header_look(theme, spec, sort, response.hovered() || menu_open);
    paint_header(ui, theme, spec, &look);
    let role = if spec.sort.is_some() {
        egui::Role::Button
    } else {
        egui::Role::ColumnHeader
    };
    response.widget_info(|| egui::WidgetInfo::labeled(role, true, spec.name));
    response
}

/// How a header cell reads: its text color and the sort arrow, if any.
struct HeaderLook {
    text: Color32,
    arrow: Option<(Icon, Color32)>,
}

fn header_look(theme: &UiTheme, spec: &ColumnSpec, sort: Sort, hot: bool) -> HeaderLook {
    let c = &theme.colors;
    let arrow = |descending: bool| {
        if descending {
            Icon::ChevronDown
        } else {
            Icon::ChevronUp
        }
    };
    match spec.sort {
        Some(key) if key == sort.key => HeaderLook {
            text: c.text_primary,
            arrow: Some((arrow(sort.descending), c.text_secondary)),
        },
        // why: a faint arrow previews the direction one click sorts in.
        Some(key) if hot => HeaderLook {
            text: c.text_primary,
            arrow: Some((arrow(next_sort(sort, key).descending), c.text_muted)),
        },
        Some(_) | None => HeaderLook {
            text: c.text_secondary,
            arrow: None,
        },
    }
}

/// The title, truncated to leave room for the arrow, which sits on the side away from the
/// alignment edge so the title never moves.
fn paint_header(ui: &egui::Ui, theme: &UiTheme, spec: &ColumnSpec, look: &HeaderLook) {
    if spec.header.is_empty() {
        return;
    }
    let cell = ui.max_rect();
    let gap = theme.tokens.metrics.space.sm;
    let reserve = if spec.sort.is_some() {
        ARROW + gap
    } else {
        0.0
    };
    let mut job =
        LayoutJob::simple_singleline(spec.header.to_owned(), theme.fonts.label.clone(), look.text);
    job.wrap = TextWrapping::truncate_at_width((cell.width() - reserve).max(0.0));
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let y = cell.center().y;
    let (text_x, arrow_x) = if spec.align_right {
        let left = cell.right() - size.x;
        (left, left - gap - ARROW / 2.0)
    } else {
        (cell.left(), cell.left() + size.x + gap + ARROW / 2.0)
    };
    ui.painter()
        .galley(egui::pos2(text_x, y - size.y / 2.0), galley, look.text);
    if let Some((icon, color)) = look.arrow {
        let rect = Rect::from_center_size(egui::pos2(arrow_x, y), egui::vec2(ARROW, ARROW));
        icons::paint(ui.painter(), rect, icon, color);
    }
}

/// Drag strips on the dividers after resizable columns (`cells` are the cells before the last
/// one), across the header only, so rows and group headers stay clickable edge to edge.
fn resize_strips(ui: &mut egui::Ui, hcx: &HeaderCx<'_>, cells: &[Rect]) -> Option<Resize> {
    let area = Rect::from_x_y_ranges(hcx.span, ui.max_rect().y_range());
    let mut strips = ui.new_child(UiBuilder::new().max_rect(area));
    strips.set_clip_rect(area);
    let mut resize = None;
    for (spec, cell) in hcx.columns.iter().zip(cells) {
        if spec.is_resizable()
            && let Some(found) = strip(&strips, hcx.theme, spec, *cell)
        {
            resize = Some(found);
        }
    }
    resize
}

/// One divider: drag to resize, double-click for the default width.
fn strip(ui: &egui::Ui, theme: &UiTheme, spec: &ColumnSpec, cell: Rect) -> Option<Resize> {
    let half_gap = ui.spacing().item_spacing.x / 2.0;
    let x = cell.right() + half_gap;
    let rect = Rect::from_x_y_ranges((x - GRAB)..=(x + GRAB), cell.y_range());
    let id = egui::Id::unique(("pr_table_resize", spec.id));
    let response = ui.interact(rect, id, Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
        let color = if response.dragged() {
            theme.colors.border_focus
        } else {
            theme.colors.border_control
        };
        let stroke = Stroke::new(theme.tokens.metrics.stroke.emphasis, color);
        ui.painter().vline(x, cell.y_range(), stroke);
    }
    if response.double_clicked() {
        return Some(Resize {
            key: spec.key,
            width: None,
        });
    }
    let pointer = response
        .interact_pointer_pos()
        .filter(|_| response.dragged())?;
    let width = (pointer.x - half_gap - cell.left()).max(spec.min).round();
    Some(Resize {
        key: spec.key,
        width: Some(width),
    })
}

/// A check per listed column (Status and Title locked on), then reset. It stays open while the
/// checks change, so several columns can be switched at once.
fn column_menu(response: &egui::Response, hcx: &HeaderCx<'_>, pending: &mut Vec<UiAction>) {
    let theme = hcx.theme;
    egui::Popup::context_menu(response)
        .frame(controls::menu_frame(theme))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(MENU_WIDTH)
        .show(|ui| {
            ui.set_width(MENU_WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            column_checks(ui, theme, &hcx.view.hidden_columns, pending);
            let space = theme.tokens.metrics.space.xs;
            ui.add_space(space);
            controls::row_divider(ui, theme);
            ui.add_space(space);
            let reset = CommandId::ColumnsReset;
            if controls::menu_item(ui, theme, reset.title(), false).clicked() {
                pending.push(UiAction::Run(reset));
                ui.close();
            }
        });
}

fn column_checks(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    hidden: &[String],
    pending: &mut Vec<UiAction>,
) {
    for column in columns::listed() {
        let locked = column.visibility() == Visibility::Locked;
        let shown = columns::chosen(&column, hidden);
        let response = ui
            .add_enabled_ui(!locked, |ui| {
                controls::menu_check(ui, theme, column.name, shown)
            })
            .inner
            .on_disabled_hover_text(copy::COLUMN_LOCKED);
        if response.clicked() {
            pending.push(UiAction::ToggleColumn(column.id));
        }
    }
}
