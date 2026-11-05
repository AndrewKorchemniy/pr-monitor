//! Where the header's cells lie and how they slide. A header cell stands for a column unit (the
//! PR cell covers Status, Number and Flags). The cells tile the whole header row, so no click
//! falls between two of them, and a dragged cell lands after as many of the others as its
//! centre has passed. While columns change places, each unit's cell and its cells in the rows
//! are drawn a little way from where the table laid them, and that distance shrinks to nothing.

use eframe::egui::{self, Rangef, Rect, UiBuilder, vec2};

use crate::view::columns::{ColumnId, ColumnSpec, ColumnUnit, UNITS};

/// A slide shorter than this has finished (points).
const REST: f32 = 0.25;

/// One header cell: a unit's columns and what their cell answers for.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct UnitCell {
    pub unit: ColumnUnit,
    /// The unit's columns' slots, side by side: where its label and its cells in the rows lie.
    pub slot: Rect,
    /// What takes the pointer: reaches the middle of the gaps either side, the first cell to
    /// the table's left edge and the last to its right edge.
    pub hit: Rect,
}

/// The header's cells, left to right.
#[derive(Copy, Clone, Debug)]
pub(super) struct UnitCells {
    cells: [Option<UnitCell>; UNITS],
}

impl UnitCells {
    /// The cells of `columns`, whose slots are `slots` (the pinned margin has none of its own;
    /// the first cell takes its place). `span` is the table's width and `rows` the header's
    /// height, which every cell's target spans.
    pub(super) fn new(
        columns: &[ColumnSpec],
        slots: &[Rect],
        (span, rows): (Rangef, Rangef),
    ) -> Self {
        let mut cells = [None; UNITS];
        let mut next = cells.iter_mut();
        let mut at = 0;
        for chunk in columns.chunk_by(|a, b| a.id.unit() == b.id.unit()) {
            let from = at;
            at += chunk.len();
            let (Some(unit), Some(own)) =
                (chunk.first().and_then(|c| c.id.unit()), slots.get(from..at))
            else {
                continue;
            };
            let slot = own.iter().fold(Rect::NOTHING, |all, s| all.union(*s));
            if let Some(cell) = next.next() {
                *cell = Some(UnitCell {
                    unit,
                    slot,
                    hit: slot,
                });
            }
        }
        let mut tiled = Self { cells };
        tiled.tile(span, rows);
        tiled
    }

    /// Stretches each cell's target to the middle of the gap before and after it.
    fn tile(&mut self, span: Rangef, rows: Rangef) {
        let slots = self.cells.map(|c| c.map(|c| c.slot));
        for (index, cell) in self.cells.iter_mut().enumerate() {
            let Some(cell) = cell else { continue };
            let before = index
                .checked_sub(1)
                .and_then(|i| slots.get(i).copied().flatten());
            let after = slots.get(index + 1).copied().flatten();
            let left = before.map_or(span.min, |b| f32::midpoint(b.right(), cell.slot.left()));
            let right = after.map_or(span.max, |a| f32::midpoint(cell.slot.right(), a.left()));
            cell.hit = Rect::from_x_y_ranges(left..=right, rows);
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &UnitCell> {
        self.cells.iter().flatten()
    }

    pub(super) fn get(&self, unit: ColumnUnit) -> Option<&UnitCell> {
        self.iter().find(|c| c.unit == unit)
    }

    /// Where `unit` stands among the shown units, from the left.
    pub(super) fn position(&self, unit: ColumnUnit) -> Option<usize> {
        self.iter().position(|c| c.unit == unit)
    }

    /// How many of the other cells a `unit` cell whose left edge is at `left` has to follow.
    /// A cell passes a neighbour once its leading edge (the right one going right, the left one
    /// going left) is past that neighbour's middle. Left where it lies it follows what it
    /// follows now; and once it has swapped with a neighbour, that neighbour's middle is behind
    /// the cell's own edge, so it never flips back and forth between two places.
    pub(super) fn among(&self, unit: ColumnUnit, left: f32) -> usize {
        let (Some(mine), Some(at)) = (self.get(unit), self.position(unit)) else {
            return 0;
        };
        let right = left + mine.slot.width();
        let ahead = self
            .iter()
            .skip(at + 1)
            .filter(|c| c.slot.center().x < right)
            .count();
        let behind = self
            .iter()
            .take(at)
            .filter(|c| c.slot.center().x > left)
            .count();
        at + ahead - behind
    }

    /// The farthest left and right a dragged cell's left edge may go: its slot stays inside
    /// the cells' row.
    pub(super) fn reach(&self, unit: ColumnUnit) -> Rangef {
        let first = self.iter().next().map_or(0.0, |c| c.slot.left());
        let last = self.iter().last().map_or(0.0, |c| c.slot.right());
        let width = self.get(unit).map_or(0.0, |c| c.slot.width());
        Rangef::new(first, (last - width).max(first))
    }
}

/// How far each unit's cells are drawn from where the table laid them (points).
#[derive(Copy, Clone, Debug, Default)]
pub(super) struct Slide {
    dx: [f32; UNITS],
}

impl Slide {
    pub(super) fn set(&mut self, unit: ColumnUnit, dx: f32) {
        if let Some(slot) = self.dx.get_mut(unit.index()) {
            *slot = dx;
        }
    }

    pub(super) fn of(&self, unit: ColumnUnit) -> f32 {
        self.dx.get(unit.index()).copied().unwrap_or(0.0)
    }

    pub(super) fn of_column(&self, column: ColumnId) -> f32 {
        column.unit().map_or(0.0, |unit| self.of(unit))
    }

    /// Whether this column's cells are away from their slot, and so drawn by `slide_in`.
    pub(super) fn sliding(&self, column: ColumnId) -> bool {
        self.of_column(column).abs() > REST
    }

    /// Whether any unit is still on its way.
    pub(super) fn moving(&self) -> bool {
        self.dx.iter().any(|dx| dx.abs() > REST)
    }
}

/// Draws a sliding column's cell in a row where its unit's slide puts it, from the row's last
/// cell. Each cell of a striped or selected row paints its fill as it is added, over whatever
/// came before, so a cell that has slid into a later column's slot must be drawn after all
/// of them. `slot` is where the table laid the column and `clip` the rows' area, which a
/// sliding cell may fill.
pub(super) fn slide_in(
    ui: &mut egui::Ui,
    (slide, clip): (&Slide, Rect),
    (column, slot): (ColumnId, Rect),
    add: impl FnOnce(&mut egui::Ui),
) {
    let rect = slot.translate(vec2(slide.of_column(column), 0.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .id_salt(("slid", column as u8)),
    );
    child.set_clip_rect(clip);
    add(&mut child);
}

/// A saved distance of this unit's header from where it was laid, for moves to start from.
pub(super) fn slide_id(unit: ColumnUnit) -> egui::Id {
    egui::Id::unique(("pr_table_slide", unit.key()))
}

/// The id of what the header remembers between frames: the order it last drew and whether
/// cells were still sliding.
pub(super) fn memory_id() -> egui::Id {
    egui::Id::unique("pr_table_slide_memory")
}

#[cfg(test)]
mod tests;
