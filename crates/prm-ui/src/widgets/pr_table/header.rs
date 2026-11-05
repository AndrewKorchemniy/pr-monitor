//! The table header. Its cells tile the whole header row, one per column unit (the PR cell
//! stands for Status, Number and Flags), and each is one target: a click sorts and previews the
//! sort arrow, a drag moves the column, a right-click opens the column menu, and the cell fills
//! on hover edge to edge. Widths change from drag strips on the dividers, which live in the
//! header only and sit on top of the cells, so a resize never starts a move.

use eframe::egui::{
    self, Color32, CursorIcon, Key, Modifiers, Rangef, Rect, Sense, Stroke, StrokeKind, UiBuilder,
    text::{LayoutJob, TextWrapping},
};
use egui_extras::TableRow;
use prm_domain::view::{Sort, SortKey};

use super::reorder::{self, Slide, UnitCell, UnitCells};
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
        columns::{self, COLUMNS, ColumnSpec, ColumnUnit, Step, Width},
        next_sort,
    },
    widgets::controls::{self, Hold},
};

/// The sort arrow.
const ARROW: f32 = 8.0;
/// Half a divider's drag strip, like the `egui_extras` resize grab.
const GRAB: f32 = 3.0;

/// A width dragged this frame, or `None` when a double-click asked for the default back.
pub(super) struct Resize {
    pub key: &'static str,
    pub width: Option<f32>,
}

/// What the header asks the table to do about moving a column.
pub(super) enum Reorder {
    /// A header was picked up, `grab` points right of its left edge.
    Lift {
        unit: ColumnUnit,
        grab: f32,
    },
    /// The lifted header now belongs after `among` of the other units.
    Retarget {
        among: usize,
    },
    Drop,
    /// The keyboard or the column menu moved a unit one place.
    Step(ColumnUnit, Step),
}

/// What one frame of the header found.
#[derive(Default)]
pub(super) struct HeaderOut {
    pub resize: Option<Resize>,
    pub reorder: Option<Reorder>,
    /// How far each unit's cells are drawn from their place (the rows follow).
    pub slide: Slide,
    /// Where the lifted header will land, for the wash over the rows.
    pub landing: Option<Rangef>,
}

pub(super) struct HeaderCx<'a> {
    pub theme: &'a UiTheme,
    /// The table's horizontal extent.
    pub span: Rangef,
    pub columns: &'a [ColumnSpec],
    pub view: &'a DashboardView,
}

/// The column menu's popup, shared by every header cell so it stays open when the column it
/// was opened from is hidden. Its temp data is the unit that opened it.
fn menu_id() -> egui::Id {
    egui::Id::unique("pr_table_column_menu")
}

pub(super) fn header_row(
    header: &mut TableRow<'_, '_>,
    hcx: &HeaderCx<'_>,
    pending: &mut Vec<UiAction>,
) -> HeaderOut {
    let last = hcx.columns.len().saturating_sub(1);
    let mut slots = [Rect::NOTHING; COLUMNS.len()];
    let mut out = HeaderOut::default();
    let mut all: Option<egui::Response> = None;
    for index in 0..hcx.columns.len() {
        header.col(|ui| {
            if index == 0 {
                header_rule(ui, hcx.theme, hcx.span);
            }
            if let Some(slot) = slots.get_mut(index) {
                *slot = ui.max_rect();
            }
            if index == last {
                // why: registered last, the cells' targets lie over the (empty) cells
                // `egui_extras` made, and the strips over those.
                all = header_cells(
                    ui,
                    hcx,
                    slots.get(..=index).unwrap_or_default(),
                    (&mut out, pending),
                );
            }
        });
    }
    // why: the trailing margin column (see `table_builder`).
    header.col(|_| {});
    if let Some(all) = all {
        column_menu(&all, hcx, (pending, &mut out.reorder));
    }
    out
}

/// Draws and answers every header cell; returns them as one response, so a right-click on any
/// opens the column menu.
fn header_cells(
    ui: &mut egui::Ui,
    hcx: &HeaderCx<'_>,
    slots: &[Rect],
    (out, pending): (&mut HeaderOut, &mut Vec<UiAction>),
) -> Option<egui::Response> {
    let row = slots.first()?.y_range();
    // why: the target reaches the hairline under the header, so no click falls between the
    // header and the first row.
    let rows = Rangef::new(row.min, row.max + ui.spacing().item_spacing.y / 2.0);
    let cells = UnitCells::new(hcx.columns, slots, (hcx.span, rows));
    let area = Rect::from_x_y_ranges(hcx.span, rows);
    let mut layer = ui.new_child(UiBuilder::new().max_rect(area));
    layer.set_clip_rect(area);
    out.slide = slide_cells(&layer, hcx, &cells);
    let mut all: Option<egui::Response> = None;
    for cell in cells.iter() {
        let response = unit_cell(&mut layer, hcx, (&cells, cell), (out, pending));
        all = Some(match all.take() {
            Some(all) => all.union(response),
            None => response,
        });
    }
    out.resize = resize_strips(&layer, hcx, slots);
    if let Some(drag) = &hcx.view.reorder
        && let Some(cell) = cells.get(drag.unit)
    {
        paint_lift(&layer, hcx, &cells, cell);
    }
    all
}

/// A hairline under the whole header row, painted from its first cell.
fn header_rule(ui: &egui::Ui, theme: &UiTheme, span: Rangef) {
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

/// Where each unit's cells are drawn this frame: the table lays them in the order it was told,
/// and a unit whose place changed slides from where it was. The distance of each from its place
/// is the slide. A change of order starts it; it runs until every cell has arrived, and a window
/// resize or a width drag is never slid.
fn slide_cells(ui: &egui::Ui, hcx: &HeaderCx<'_>, cells: &UnitCells) -> Slide {
    let order = order_signature(hcx.view.layout_order());
    let id = reorder::memory_id();
    let (before, sliding) = ui
        .ctx()
        .data(|d| d.get_temp::<(u64, bool)>(id))
        .unwrap_or((order, false));
    let time = if hcx.theme.policy.animations && (sliding || before != order) {
        hcx.theme.tokens.metrics.motion.reorder
    } else {
        0.0
    };
    let mut slide = Slide::default();
    for cell in cells.iter() {
        let target = cell.slot.left() - hcx.span.min;
        let shown = ui
            .ctx()
            .animate_value_with_time(reorder::slide_id(cell.unit), target, time);
        slide.set(cell.unit, shown - target);
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(id, (order, slide.moving())));
    slide
}

/// The units' order as one number, to tell when it changed.
fn order_signature(order: &[ColumnUnit]) -> u64 {
    order.iter().fold(0, |hash, unit| {
        hash.rotate_left(5) ^ (unit.index() as u64 + 1)
    })
}

/// One header cell and what clicking, dragging or focusing it asks for.
fn unit_cell(
    ui: &mut egui::Ui,
    hcx: &HeaderCx<'_>,
    (cells, cell): (&UnitCells, &UnitCell),
    (out, pending): (&mut HeaderOut, &mut Vec<UiAction>),
) -> egui::Response {
    let theme = hcx.theme;
    let unit = cell.unit;
    let id = egui::Id::unique(("pr_table_header", unit.key()));
    let response = ui.interact(cell.hit, id, Sense::click_and_drag());
    let menu_open = menu_owner(ui.ctx()) == Some(unit);
    let lifted = hcx.view.reorder.as_ref().is_some_and(|d| d.unit == unit);
    let dx = out.slide.of(unit);
    if lifted {
        paint_landing(ui, hcx, cell, dx, &mut out.landing);
    } else {
        let hold = if menu_open {
            Hold::MenuOpen
        } else {
            Hold::Free
        };
        controls::flat_chrome(ui, theme, cell.hit, &response, hold);
        let hot = response.hovered() || menu_open;
        let look = header_look(theme, unit, hcx.view.sort, hot);
        paint_label(
            ui,
            theme,
            unit,
            cell.slot.translate(egui::vec2(dx, 0.0)),
            &look,
        );
    }
    unit_events(ui, hcx, (cells, cell), &response, (out, pending));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, unit.label()));
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_description(copy::COLUMN_REORDER_HINT);
    });
    response
}

/// What a cell's response asks for: the sort on a click, the menu on a right-click, a move from
/// a drag or Alt with an arrow key.
fn unit_events(
    ui: &egui::Ui,
    hcx: &HeaderCx<'_>,
    (cells, cell): (&UnitCells, &UnitCell),
    response: &egui::Response,
    (out, pending): (&mut HeaderOut, &mut Vec<UiAction>),
) {
    let unit = cell.unit;
    if response.clicked() {
        pending.push(UiAction::Sort(unit.sort()));
    }
    if response.secondary_clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(menu_id(), unit));
    }
    if response.drag_started() {
        let press = ui.input(|i| i.pointer.press_origin());
        let grab = press.map_or(0.0, |p| p.x - cell.slot.left());
        out.reorder = Some(Reorder::Lift { unit, grab });
    } else if response.drag_stopped() {
        out.reorder = Some(Reorder::Drop);
    } else if response.dragged() {
        out.reorder = retarget(ui, hcx, (cells, cell)).or(out.reorder.take());
    } else if response.has_focus() {
        out.reorder = key_step(ui, unit).or(out.reorder.take());
    }
}

/// Where the pointer has carried the lifted header: `Retarget` once it has passed another cell's
/// middle.
fn retarget(
    ui: &egui::Ui,
    hcx: &HeaderCx<'_>,
    (cells, cell): (&UnitCells, &UnitCell),
) -> Option<Reorder> {
    let drag = hcx.view.reorder.as_ref().filter(|d| d.unit == cell.unit)?;
    let left = lifted_left(ui, drag.grab, cells.reach(cell.unit))?;
    let among = cells.among(cell.unit, left);
    (cells.position(cell.unit) != Some(among)).then_some(Reorder::Retarget { among })
}

/// The lifted cell's left edge: under the pointer, kept within the row.
fn lifted_left(ui: &egui::Ui, grab: f32, reach: Rangef) -> Option<f32> {
    let x = ui.input(|i| i.pointer.latest_pos())?.x;
    Some((x - grab).clamp(reach.min, reach.max))
}

/// Alt with Left or Right on a focused cell moves its column.
fn key_step(ui: &egui::Ui, unit: ColumnUnit) -> Option<Reorder> {
    let pressed = |key| ui.input_mut(|i| i.consume_key(Modifiers::ALT, key));
    if pressed(Key::ArrowLeft) {
        Some(Reorder::Step(unit, Step::Left))
    } else if pressed(Key::ArrowRight) {
        Some(Reorder::Step(unit, Step::Right))
    } else {
        None
    }
}

/// The empty place a lifted header leaves: a wash the width of its cell, which the rows' wash
/// continues (`HeaderOut::landing`).
fn paint_landing(
    ui: &egui::Ui,
    hcx: &HeaderCx<'_>,
    cell: &UnitCell,
    dx: f32,
    landing: &mut Option<Rangef>,
) {
    let gap = ui.spacing().item_spacing.x;
    let x = (cell.slot.left() + dx - gap / 2.0)..=(cell.slot.right() + dx + gap / 2.0);
    let rect = Rect::from_x_y_ranges(x.clone(), cell.hit.y_range());
    controls::wash(ui.painter(), hcx.theme, rect);
    *landing = Some(Rangef::from(x));
}

/// The header held by the pointer, over the others: a raised cell at the pointer's x.
fn paint_lift(ui: &egui::Ui, hcx: &HeaderCx<'_>, cells: &UnitCells, cell: &UnitCell) {
    let theme = hcx.theme;
    let Some(drag) = &hcx.view.reorder else {
        return;
    };
    let Some(left) = lifted_left(ui, drag.grab, cells.reach(cell.unit)) else {
        return;
    };
    let gap = ui.spacing().item_spacing.x;
    let shift = egui::vec2(left - cell.slot.left(), 0.0);
    let body = cell
        .slot
        .expand2(egui::vec2(gap / 2.0, 0.0))
        .translate(shift);
    let rect = Rect::from_x_y_ranges(body.x_range(), cell.hit.y_range());
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter().rect(
        rect,
        0.0,
        theme.colors.surface_overlay,
        stroke,
        StrokeKind::Inside,
    );
    let look = header_look(theme, cell.unit, hcx.view.sort, true);
    paint_label(ui, theme, cell.unit, cell.slot.translate(shift), &look);
}

/// The column whose header opened the column menu, while it is open.
fn menu_owner(ctx: &egui::Context) -> Option<ColumnUnit> {
    if !egui::Popup::is_id_open(ctx, menu_id()) {
        return None;
    }
    ctx.data(|d| d.get_temp(menu_id()))
}

/// How a header cell reads: its text color and the sort arrow, if any.
struct HeaderLook {
    text: Color32,
    arrow: Option<(Icon, Color32)>,
}

fn header_look(theme: &UiTheme, unit: ColumnUnit, sort: Sort, hot: bool) -> HeaderLook {
    let c = &theme.colors;
    let arrow = |descending: bool| {
        if descending {
            Icon::ChevronDown
        } else {
            Icon::ChevronUp
        }
    };
    let key: SortKey = unit.sort();
    if key == sort.key {
        HeaderLook {
            text: c.text_primary,
            arrow: Some((arrow(sort.descending), c.text_secondary)),
        }
    } else if hot {
        // why: a faint arrow previews the direction one click sorts in.
        HeaderLook {
            text: c.text_primary,
            arrow: Some((arrow(next_sort(sort, key).descending), c.text_muted)),
        }
    } else {
        HeaderLook {
            text: c.text_secondary,
            arrow: None,
        }
    }
}

/// The title, truncated to leave room for the arrow, which sits on the side away from the
/// alignment edge so the title never moves. `slot` is where the unit's columns lie, so the text
/// keeps its own margin whatever the cell's target reaches.
fn paint_label(ui: &egui::Ui, theme: &UiTheme, unit: ColumnUnit, slot: Rect, look: &HeaderLook) {
    let gap = theme.tokens.metrics.space.sm;
    let mut job = LayoutJob::simple_singleline(
        unit.label().to_owned(),
        theme.fonts.label.clone(),
        look.text,
    );
    job.wrap = TextWrapping::truncate_at_width((slot.width() - ARROW - gap).max(0.0));
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let y = slot.center().y;
    let (text_x, arrow_x) = if unit.align_right() {
        let left = slot.right() - size.x;
        (left, left - gap - ARROW / 2.0)
    } else {
        (slot.left(), slot.left() + size.x + gap + ARROW / 2.0)
    };
    ui.painter()
        .galley(egui::pos2(text_x, y - size.y / 2.0), galley, look.text);
    if let Some((icon, color)) = look.arrow {
        let rect = Rect::from_center_size(egui::pos2(arrow_x, y), egui::vec2(ARROW, ARROW));
        icons::paint(ui.painter(), rect, icon, color);
    }
}

/// Which divider of its cell a column's drag strip sits on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Divider {
    /// Columns before Title: dragging right widens them.
    Right,
    /// Columns after Title: dragging left widens them. Title gives up the room, so the divider
    /// stays under the pointer, and the last column gets a strip too.
    Left,
}

/// Where one column's drag strip sits and how far it may go.
struct Grip {
    cell: Rect,
    divider: Divider,
    /// The widest the column may get before Title would drop under its minimum (points).
    max: f32,
}

/// Drag strips on the dividers of resizable columns (`cells` are this frame's slots), across
/// the header only, so rows and group headers stay clickable edge to edge.
fn resize_strips(ui: &egui::Ui, hcx: &HeaderCx<'_>, cells: &[Rect]) -> Option<Resize> {
    let flex = hcx.columns.iter().position(|c| c.width == Width::Remainder);
    // why: a column grows only into what Title has above its minimum, so the table never
    // runs past its right edge.
    let slack = flex
        .and_then(|i| Some(cells.get(i)?.width() - hcx.columns.get(i)?.min))
        .unwrap_or(0.0)
        .max(0.0);
    let mut resize = None;
    for (index, (spec, cell)) in hcx.columns.iter().zip(cells).enumerate() {
        let divider = if flex.is_some_and(|flex| index > flex) {
            Divider::Left
        } else {
            Divider::Right
        };
        let grip = Grip {
            cell: *cell,
            divider,
            max: (cell.width() + slack).floor(),
        };
        if spec.is_resizable()
            && let Some(found) = strip(ui, hcx.theme, spec, &grip)
        {
            resize = Some(found);
        }
    }
    resize
}

/// One divider: drag to resize, double-click for the default width.
fn strip(ui: &egui::Ui, theme: &UiTheme, spec: &ColumnSpec, grip: &Grip) -> Option<Resize> {
    let half_gap = ui.spacing().item_spacing.x / 2.0;
    let x = match grip.divider {
        Divider::Right => grip.cell.right() + half_gap,
        Divider::Left => grip.cell.left() - half_gap,
    };
    let rect = Rect::from_x_y_ranges((x - GRAB)..=(x + GRAB), grip.cell.y_range());
    let id = egui::Id::unique(("pr_table_resize", spec.id));
    // why: clicks for the double-click reset, but no Tab stop: "Reset columns" covers keyboards.
    let response = ui.interact(rect, id, Sense::click_and_drag() - Sense::FOCUSABLE);
    paint_strip(ui, theme, x, grip.cell, &response);
    if response.double_clicked() {
        return Some(Resize {
            key: spec.key,
            width: None,
        });
    }
    if response.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(id, grip.cell.width()));
    }
    let moved = response.total_drag_delta()?.x;
    // why: measured from the width the drag began at, not from this frame's cell, whose far
    // edge moves as Title gives or takes the room; that fed back and ran away.
    let start = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(id))
        .unwrap_or_else(|| grip.cell.width());
    let grown = match grip.divider {
        Divider::Right => moved,
        Divider::Left => -moved,
    };
    Some(Resize {
        key: spec.key,
        width: Some((start + grown).min(grip.max).max(spec.min).round()),
    })
}

/// The divider line and the resize cursor while a strip is hovered or dragged.
fn paint_strip(ui: &egui::Ui, theme: &UiTheme, x: f32, cell: Rect, response: &egui::Response) {
    if !response.hovered() && !response.dragged() {
        return;
    }
    ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
    let color = if response.dragged() {
        theme.colors.border_focus
    } else {
        theme.colors.border_control
    };
    let stroke = Stroke::new(theme.tokens.metrics.stroke.emphasis, color);
    ui.painter().vline(x, cell.y_range(), stroke);
}

/// What the column menu offers for the cell it was opened from, then a check per listed column
/// (Status and Title locked on), then reset. It stays open while the checks change, so several
/// columns can be switched at once, the one it was opened from too.
fn column_menu(
    all: &egui::Response,
    hcx: &HeaderCx<'_>,
    (pending, reorder): (&mut Vec<UiAction>, &mut Option<Reorder>),
) {
    let theme = hcx.theme;
    egui::Popup::context_menu(all)
        .id(menu_id())
        .frame(controls::menu_frame(theme))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(controls::MENU_WIDTH)
        .show(|ui| {
            ui.set_width(controls::MENU_WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(owner) = menu_owner(ui.ctx()) {
                owner_items(ui, hcx, owner, (pending, reorder));
                controls::spaced_divider(ui, theme, theme.tokens.metrics.space.xs);
            }
            column_checks(ui, hcx, pending);
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

/// The items for one header cell: moving it, and for PR the status sort its click lacks.
fn owner_items(
    ui: &mut egui::Ui,
    hcx: &HeaderCx<'_>,
    owner: ColumnUnit,
    (pending, reorder): (&mut Vec<UiAction>, &mut Option<Reorder>),
) {
    let theme = hcx.theme;
    if owner == ColumnUnit::Pr
        && controls::menu_item(ui, theme, copy::SORT_BY_STATUS, false).clicked()
    {
        pending.push(UiAction::Sort(SortKey::Status));
        ui.close();
    }
    let mut shown = hcx.columns.iter().filter_map(|c| c.id.unit());
    let first = shown.next();
    let last = shown.next_back().or(first);
    let moves = [
        (copy::move_column_left(owner.label()), Step::Left, first),
        (copy::move_column_right(owner.label()), Step::Right, last),
    ];
    for (label, step, end) in moves {
        let movable = end != Some(owner);
        let item = ui.add_enabled_ui(movable, |ui| controls::menu_item(ui, theme, &label, false));
        if item.inner.clicked() {
            *reorder = Some(Reorder::Step(owner, step));
        }
    }
}

fn column_checks(ui: &mut egui::Ui, hcx: &HeaderCx<'_>, pending: &mut Vec<UiAction>) {
    let theme = hcx.theme;
    let choices = hcx.view.column_choices();
    for column in columns::listed() {
        let response = super::column_choice(
            ui,
            theme,
            (&column, choices.check(&column), true),
            |ui, state, label| controls::menu_check(ui, theme, state, label),
        );
        if response.clicked() {
            pending.push(UiAction::ToggleColumn(column.id));
        }
    }
}
