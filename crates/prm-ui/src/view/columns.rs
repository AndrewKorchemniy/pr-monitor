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

    /// The least room the column takes: its dragged or default width, Title its minimum, and
    /// the margin `margin` (points).
    fn narrowest(&self, margin: f32, saved: Option<f32>) -> f32 {
        match self.width {
            Width::Exact => margin,
            Width::Fixed(width) => width,
            Width::Remainder => self.min,
            Width::Initial(width) => saved.unwrap_or(width).max(self.min),
        }
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

impl ColumnLayout<'_> {
    fn wants(&self, column: &ColumnSpec) -> bool {
        match column.visibility() {
            Visibility::BySetting => self.show_org,
            Visibility::Unlisted | Visibility::Locked | Visibility::Optional => {
                chosen(column, self.hidden)
            }
        }
    }

    /// The room `columns` need side by side, Title at its minimum (points).
    fn needed(&self, columns: &[ColumnSpec], room: Room) -> f32 {
        let spaced: f32 = columns
            .iter()
            .map(|c| c.narrowest(room.margin, self.widths.get(c.key).copied()) + room.gap)
            .sum();
        spaced - room.gap
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

/// Columns in order: the user's choice, then, while they need more room than there is, the
/// next column in `SQUEEZE_ORDER` drops out. So hiding a column leaves room for others, and
/// the table never runs past its right edge.
pub(crate) fn visible_columns(room: Room, layout: &ColumnLayout<'_>) -> Vec<ColumnSpec> {
    let mut columns: Vec<ColumnSpec> = COLUMNS.into_iter().filter(|c| layout.wants(c)).collect();
    if layout.group == GroupBy::Status {
        compact_status(&mut columns);
    }
    for id in SQUEEZE_ORDER {
        if layout.needed(&columns, room) <= room.width {
            break;
        }
        columns.retain(|c| c.id != id);
    }
    columns
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
