//! Table columns: ids, persisted keys, default widths, sorting, who may hide each one, and which
//! show at a width.

use prm_domain::view::SortKey;

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

    /// Whether the column fits the window: narrow windows drop the widest optional columns.
    fn fits(self, width: f32, show_org: bool) -> bool {
        match self {
            Self::Org => show_org && width >= NARROW,
            Self::Reviews => width >= NARROW,
            Self::Repo => width >= NARROWER,
            Self::Margin
            | Self::Status
            | Self::Number
            | Self::Flags
            | Self::Title
            | Self::Author
            | Self::Comments
            | Self::Age => true,
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
    /// Only `Initial` columns get a drag strip on their right divider.
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

/// Below this width Reviews and Organization hide.
pub(crate) const NARROW: f32 = 900.0;
/// Below this width Repository hides too.
pub(crate) const NARROWER: f32 = 760.0;

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

/// Columns in order for the available width: the user's choice (`hidden` keys), then what
/// fits.
pub(crate) fn visible_columns(width: f32, show_org: bool, hidden: &[String]) -> Vec<ColumnSpec> {
    COLUMNS
        .into_iter()
        .filter(|c| chosen(c, hidden))
        .filter(|c| c.id.fits(width, show_org))
        .collect()
}

/// A stable id for the visible set, so `egui_extras` width state never mismatches the count.
pub(crate) fn columns_hash(columns: &[ColumnSpec]) -> u64 {
    columns
        .iter()
        .fold(0_u64, |hash, c| hash.rotate_left(5) ^ (c.id as u64 + 1))
}

#[cfg(test)]
mod tests;
