//! Table columns: ids, persisted keys, default widths, sorting, who may hide each one, and which
//! the table has room for.

use std::collections::BTreeMap;

use prm_domain::view::{GroupBy, SortKey};

use crate::copy;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ColumnId {
    /// The change bar.
    Margin,
    Status,
    Number,
    /// Draft, merge conflicts and auto-complete marks, so they never push the title along.
    Flags,
    Title,
    Repo,
    Author,
    Reviews,
    Comments,
    Age,
    Org,
}

/// Who decides whether a column shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Visibility {
    /// Always shown and never listed: the change-bar margin.
    Unlisted,
    /// Listed in the column menu but always shown (greyed out): the table needs it.
    Locked,
    /// Listed; the user may hide it.
    Optional,
    /// Not listed: Settings › General › Organization column decides.
    BySetting,
}

impl Visibility {
    /// Whether the column menu and the settings checklist list it.
    pub(crate) const fn listed(self) -> bool {
        match self {
            Self::Locked | Self::Optional => true,
            Self::Unlisted | Self::BySetting => false,
        }
    }
}

impl ColumnId {
    /// Who decides whether this column shows; the one place each column's rule lives.
    pub(crate) const fn visibility(self) -> Visibility {
        match self {
            Self::Margin => Visibility::Unlisted,
            Self::Status | Self::Title => Visibility::Locked,
            Self::Number
            | Self::Flags
            | Self::Repo
            | Self::Author
            | Self::Reviews
            | Self::Comments
            | Self::Age => Visibility::Optional,
            Self::Org => Visibility::BySetting,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Width {
    /// The margin, at the theme's margin width.
    Exact,
    /// Always this width (points); the user cannot resize it.
    Fixed(f32),
    /// Takes the remaining space, at least `min`; it flexes with the window.
    Remainder,
    /// User-resizable from the header, starting at `initial` (points).
    Initial(f32),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ColumnSpec {
    pub id: ColumnId,
    /// Persisted in `UiState.columns` / `hidden_columns`; empty for the margin.
    pub key: &'static str,
    /// What the header shows; may be empty when the cells explain themselves.
    pub header: &'static str,
    /// The column's name in the column menu and for screen readers.
    pub name: &'static str,
    pub width: Width,
    pub min: f32,
    pub sort: Option<SortKey>,
    pub align_right: bool,
}

impl ColumnSpec {
    /// Only `Initial` columns get a drag strip, on their divider away from Title.
    pub(crate) const fn is_resizable(&self) -> bool {
        matches!(self.width, Width::Initial(_))
    }

    pub(crate) const fn visibility(&self) -> Visibility {
        self.id.visibility()
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a const constructor for the column table"
)]
const fn spec(
    id: ColumnId,
    key: &'static str,
    header: &'static str,
    width: Width,
    min: f32,
    sort: Option<SortKey>,
) -> ColumnSpec {
    ColumnSpec {
        id,
        key,
        header,
        name: header,
        width,
        min,
        sort,
        align_right: false,
    }
}

const fn right(mut column: ColumnSpec) -> ColumnSpec {
    column.align_right = true;
    column
}

/// A column whose header stays blank; it keeps its name for the menu and screen readers.
const fn blank_header(mut column: ColumnSpec) -> ColumnSpec {
    column.header = "";
    column
}

/// One glyph slot in the Flags column (points).
pub(crate) const FLAG_SLOT: f32 = 14.0;
/// The Flags column: a slot each for draft, merge conflicts and auto-complete.
pub(crate) const FLAGS_WIDTH: f32 = 3.0 * FLAG_SLOT;

pub(crate) const COLUMNS: [ColumnSpec; 11] = [
    spec(ColumnId::Margin, "", "", Width::Exact, 0.0, None),
    spec(
        ColumnId::Status,
        "status",
        copy::COLUMN_STATUS,
        Width::Initial(140.0),
        30.0,
        Some(SortKey::Status),
    ),
    right(spec(
        ColumnId::Number,
        "number",
        copy::COLUMN_NUMBER,
        Width::Initial(64.0),
        52.0,
        Some(SortKey::Number),
    )),
    blank_header(spec(
        ColumnId::Flags,
        "flags",
        copy::COLUMN_FLAGS,
        Width::Fixed(FLAGS_WIDTH),
        FLAGS_WIDTH,
        None,
    )),
    spec(
        ColumnId::Title,
        "title",
        copy::COLUMN_TITLE,
        Width::Remainder,
        200.0,
        Some(SortKey::Title),
    ),
    spec(
        ColumnId::Repo,
        "repo",
        copy::COLUMN_REPO,
        Width::Initial(160.0),
        80.0,
        Some(SortKey::Repo),
    ),
    spec(
        ColumnId::Author,
        "author",
        copy::COLUMN_AUTHOR,
        Width::Initial(150.0),
        80.0,
        Some(SortKey::Author),
    ),
    spec(
        ColumnId::Reviews,
        "reviews",
        copy::REVIEWS,
        Width::Initial(84.0),
        56.0,
        None,
    ),
    right(spec(
        ColumnId::Comments,
        "comments",
        copy::COLUMN_COMMENTS,
        Width::Initial(76.0),
        56.0,
        Some(SortKey::Comments),
    )),
    right(spec(
        ColumnId::Age,
        "age",
        copy::COLUMN_AGE,
        Width::Initial(56.0),
        44.0,
        Some(SortKey::Age),
    )),
    spec(
        ColumnId::Org,
        "org",
        copy::ORGANIZATION,
        Width::Initial(120.0),
        80.0,
        Some(SortKey::Org),
    ),
];

/// What a table too narrow for its columns drops, first to last, until the rest fit. Status,
/// Number, Flags, Title and Age always stay.
const SQUEEZE_ORDER: [ColumnId; 5] = [
    ColumnId::Org,
    ColumnId::Reviews,
    ColumnId::Repo,
    ColumnId::Author,
    ColumnId::Comments,
];

/// The Status column's width when grouped by status, where only the glyph shows (points).
const STATUS_GLYPH: f32 = 24.0;
/// The PR number's width when it leads the compact layout (points).
const NUMBER_COMPACT: f32 = 56.0;

/// The space the table lays its columns out in (points).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Room {
    /// What the columns share: the table's width less the scroll bar's lane.
    pub width: f32,
    /// The space between two columns.
    pub gap: f32,
    /// The change-bar margin's width.
    pub margin: f32,
}

/// The user's choices that shape the table's columns.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ColumnLayout<'a> {
    /// Grouped by status, the Status column shows only its glyph (`compact_status`).
    pub group: GroupBy,
    /// Settings › General › Organization column.
    pub show_org: bool,
    /// Column keys the user hid.
    pub hidden: &'a [String],
    /// Widths the user dragged, by key (points).
    pub widths: &'a BTreeMap<String, f32>,
}

/// How far a resizable column may shrink to fit the window.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Give {
    /// To its default width, or its dragged width when that is narrower: a dragged width gives
    /// way before any column drops, and no column is squeezed below its default while one can.
    ToDefault,
    /// To its minimum, once only the columns that never drop are left.
    ToMinimum,
}

/// The room a column, or columns side by side, would like and the least they can do with
/// (points).
#[derive(Copy, Clone, Debug, PartialEq)]
struct Span {
    preferred: f32,
    floor: f32,
}

impl Span {
    const fn fixed(width: f32) -> Self {
        Self {
            preferred: width,
            floor: width,
        }
    }
}

impl ColumnLayout<'_> {
    fn wants(&self, column: &ColumnSpec) -> bool {
        match column.visibility() {
            Visibility::BySetting => self.show_org,
            Visibility::Unlisted | Visibility::Locked | Visibility::Optional => {
                chosen(column, self.hidden)
            }
        }
    }

    /// A column's room: its dragged or default width, Title its minimum and the margin
    /// `room.margin`.
    fn span(&self, column: &ColumnSpec, room: Room, give: Give) -> Span {
        match column.width {
            Width::Exact => Span::fixed(room.margin),
            Width::Fixed(width) => Span::fixed(width),
            Width::Remainder => Span::fixed(column.min),
            Width::Initial(default) => {
                let saved = self.widths.get(column.key).copied();
                let preferred = saved.unwrap_or(default).max(column.min);
                let floor = match give {
                    Give::ToDefault => preferred.min(default),
                    Give::ToMinimum => column.min,
                };
                Span { preferred, floor }
            }
        }
    }

    /// The room `columns` need side by side, gaps included.
    fn needed<'c>(
        &self,
        columns: impl IntoIterator<Item = &'c ColumnSpec>,
        room: Room,
        give: Give,
    ) -> Span {
        let mut total = Span::fixed(0.0);
        for (index, column) in columns.into_iter().enumerate() {
            let gap = if index == 0 { 0.0 } else { room.gap };
            let span = self.span(column, room, give);
            total.preferred += span.preferred + gap;
            total.floor += span.floor + gap;
        }
        total
    }

    /// Gives each resizable column this frame's width: the preferred ones while they fit, else
    /// each gives up the same share of its room above its floor until they do. The dragged
    /// widths themselves stay as the user left them.
    fn fit_widths(&self, columns: &mut [ColumnSpec], room: Room, give: Give) {
        let needed = self.needed(columns.iter(), room, give);
        let spare = needed.preferred - needed.floor;
        let share = if needed.preferred <= room.width || spare <= 0.0 {
            1.0
        } else {
            ((room.width - needed.floor) / spare).clamp(0.0, 1.0)
        };
        for column in columns.iter_mut() {
            if column.is_resizable() {
                let span = self.span(column, room, give);
                let width = span.floor + (span.preferred - span.floor) * share;
                column.width = Width::Initial(width.floor().max(column.min));
            }
        }
    }
}

/// The column with this id, from the table above.
pub(crate) fn column(id: ColumnId) -> Option<ColumnSpec> {
    COLUMNS.into_iter().find(|c| c.id == id)
}

/// The columns the column menu and the settings checklist offer, in table order.
pub(crate) fn listed() -> impl Iterator<Item = ColumnSpec> {
    COLUMNS.into_iter().filter(|c| c.visibility().listed())
}

/// Whether the user's choice keeps `column` shown: only optional columns can be hidden, so a
/// hand-edited `hidden` never drops Status or Title.
pub(crate) fn chosen(column: &ColumnSpec, hidden: &[String]) -> bool {
    match column.visibility() {
        Visibility::Optional => !hidden.iter().any(|h| h == column.key),
        Visibility::Unlisted | Visibility::Locked | Visibility::BySetting => true,
    }
}

/// Columns in order, each resizable one at this frame's width (`Width::Initial`). First the
/// user's choice; while that needs more room than there is, dragged widths give way (down to
/// the defaults), then the next column in `SQUEEZE_ORDER` drops out, and once only the core is
/// left its columns shrink to their minimums. So hiding a column leaves room for others, and
/// the table never runs past its right edge while its core fits.
pub(crate) fn visible_columns(room: Room, layout: &ColumnLayout<'_>) -> Vec<ColumnSpec> {
    let (mut columns, give) = pick_columns(room, layout);
    layout.fit_widths(&mut columns, room, give);
    columns
}

/// The columns `visible_columns` keeps, at their default widths, and how far they give.
fn pick_columns(room: Room, layout: &ColumnLayout<'_>) -> (Vec<ColumnSpec>, Give) {
    let mut columns: Vec<ColumnSpec> = COLUMNS.into_iter().filter(|c| layout.wants(c)).collect();
    if layout.group == GroupBy::Status {
        compact_status(&mut columns);
    }
    let fits =
        |columns: &[ColumnSpec]| layout.needed(columns, room, Give::ToDefault).floor <= room.width;
    for id in SQUEEZE_ORDER {
        if fits(&columns) {
            break;
        }
        columns.retain(|c| c.id != id);
    }
    let give = if fits(&columns) {
        Give::ToDefault
    } else {
        Give::ToMinimum
    };
    (columns, give)
}

/// The resizable columns `visible_columns` shows narrower than the user's width for them (or
/// their default), by key, at the width shown.
fn squeezed_widths(room: Room, layout: &ColumnLayout<'_>) -> Vec<(&'static str, f32)> {
    let (picked, give) = pick_columns(room, layout);
    let mut shown = picked.clone();
    layout.fit_widths(&mut shown, room, give);
    picked
        .iter()
        .zip(&shown)
        .filter_map(|(column, fitted)| match fitted.width {
            Width::Initial(width) if width < layout.span(column, room, give).preferred - 0.5 => {
                Some((column.key, width))
            }
            Width::Initial(_) | Width::Exact | Width::Fixed(_) | Width::Remainder => None,
        })
        .collect()
}

/// The widths to save when the header drags `key` to `width` points. In a table squeezed to
/// fit, the other columns keep the widths they show first; else the drag's width would be
/// squeezed again, so the divider would land short of the pointer and the other columns grow.
pub(crate) fn dragged_widths(
    room: Room,
    layout: &ColumnLayout<'_>,
    (key, width): (&'static str, f32),
) -> Vec<(&'static str, f32)> {
    let mut saved = squeezed_widths(room, layout);
    saved.retain(|(shown, _)| *shown != key);
    saved.push((key, width));
    saved
}

/// The room a table worth showing needs (`room.width` aside): its columns that never drop and
/// the last one that does (Comments), at their default widths, ungrouped, with Title at its
/// minimum. Narrower, the dashboard lists cards instead, since a table of only its core
/// columns would hide the repository, author, reviews and comments that a card still shows.
pub(crate) fn table_minimum(room: Room) -> f32 {
    let defaults = BTreeMap::new();
    let layout = ColumnLayout {
        group: GroupBy::None,
        show_org: false,
        hidden: &[],
        widths: &defaults,
    };
    let last = SQUEEZE_ORDER.last().copied();
    let minimal = COLUMNS
        .iter()
        .filter(|c| !SQUEEZE_ORDER.contains(&c.id) || Some(c.id) == last);
    layout.needed(minimal, room, Give::ToDefault).floor
}

/// Grouped by status, the group header already names it: the status cell shows only its glyph.
fn compact_status(columns: &mut [ColumnSpec]) {
    for spec in columns.iter_mut() {
        match spec.id {
            ColumnId::Status => {
                spec.header = "";
                spec.width = Width::Fixed(STATUS_GLYPH);
                spec.min = STATUS_GLYPH;
                spec.sort = None;
            }
            ColumnId::Number => {
                spec.key = "number_left";
                spec.align_right = false;
                spec.width = Width::Initial(NUMBER_COMPACT);
            }
            ColumnId::Margin
            | ColumnId::Flags
            | ColumnId::Title
            | ColumnId::Repo
            | ColumnId::Author
            | ColumnId::Reviews
            | ColumnId::Comments
            | ColumnId::Age
            | ColumnId::Org => {}
        }
    }
}

/// Listed columns the user chose that `visible` (this frame's columns) had no room for.
pub(crate) fn squeezed<'a>(
    visible: &'a [ColumnSpec],
    hidden: &'a [String],
) -> impl Iterator<Item = ColumnId> + 'a {
    listed()
        .filter(move |c| chosen(c, hidden) && !visible.iter().any(|v| v.id == c.id))
        .map(|c| c.id)
}

/// How a listed column's check reads in the column menu and the settings checklist.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColumnCheck {
    /// The user hid it.
    Off,
    /// In the table.
    On,
    /// Chosen, but the table has no room for it right now.
    Squeezed,
}

/// The user's column choices and what the table's width overrode last frame.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Choices<'a> {
    /// Column keys the user hid.
    pub hidden: &'a [String],
    /// Chosen columns the table had no room for last frame (`squeezed`).
    pub squeezed: &'a [ColumnId],
}

impl Choices<'_> {
    pub(crate) fn check(&self, column: &ColumnSpec) -> ColumnCheck {
        if !chosen(column, self.hidden) {
            ColumnCheck::Off
        } else if self.squeezed.contains(&column.id) {
            ColumnCheck::Squeezed
        } else {
            ColumnCheck::On
        }
    }
}

/// A stable id for the visible set, so `egui_extras` width state never mismatches the count.
pub(crate) fn columns_hash(columns: &[ColumnSpec]) -> u64 {
    columns
        .iter()
        .fold(0_u64, |hash, c| hash.rotate_left(5) ^ (c.id as u64 + 1))
}

#[cfg(test)]
mod tests;
