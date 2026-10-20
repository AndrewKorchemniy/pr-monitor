//! Table columns: ids, persisted keys, default widths, sorting, and which show at a width.

use prm_domain::view::SortKey;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ColumnId {
    /// The change bar.
    Margin,
    Status,
    Number,
    Title,
    Repo,
    Author,
    Reviews,
    Comments,
    Age,
    Org,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Width {
    /// Fixed (the margin).
    Exact,
    /// Takes the remaining space, at least `min`.
    Remainder,
    /// User-resizable, starting at `initial`.
    Initial(f32),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ColumnSpec {
    pub id: ColumnId,
    /// Persisted in `UiState.columns` / `hidden_columns`; empty for the margin.
    pub key: &'static str,
    pub header: &'static str,
    pub width: Width,
    pub min: f32,
    pub sort: Option<SortKey>,
    pub align_right: bool,
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

pub(crate) const COLUMNS: [ColumnSpec; 10] = [
    spec(ColumnId::Margin, "", "", Width::Exact, 0.0, None),
    spec(
        ColumnId::Status,
        "status",
        "Status",
        Width::Initial(140.0),
        30.0,
        Some(SortKey::Status),
    ),
    right(spec(
        ColumnId::Number,
        "number",
        "PR",
        Width::Initial(64.0),
        52.0,
        Some(SortKey::Number),
    )),
    spec(
        ColumnId::Title,
        "title",
        "Title",
        Width::Remainder,
        200.0,
        Some(SortKey::Title),
    ),
    spec(
        ColumnId::Repo,
        "repo",
        "Repository",
        Width::Initial(160.0),
        80.0,
        Some(SortKey::Repo),
    ),
    spec(
        ColumnId::Author,
        "author",
        "Author",
        Width::Initial(150.0),
        80.0,
        Some(SortKey::Author),
    ),
    spec(
        ColumnId::Reviews,
        "reviews",
        "Reviews",
        Width::Initial(84.0),
        56.0,
        None,
    ),
    right(spec(
        ColumnId::Comments,
        "comments",
        "Comments",
        Width::Initial(76.0),
        56.0,
        Some(SortKey::Comments),
    )),
    right(spec(
        ColumnId::Age,
        "age",
        "Age",
        Width::Initial(56.0),
        44.0,
        Some(SortKey::Age),
    )),
    spec(
        ColumnId::Org,
        "org",
        "Organization",
        Width::Initial(120.0),
        80.0,
        Some(SortKey::Org),
    ),
];

/// Below this width Reviews and Organization hide.
pub(crate) const NARROW: f32 = 900.0;
/// Below this width Repository hides too.
pub(crate) const NARROWER: f32 = 760.0;

/// Columns in order for the available width; `hidden` are user-hidden keys.
pub(crate) fn visible_columns(width: f32, show_org: bool, hidden: &[String]) -> Vec<ColumnSpec> {
    COLUMNS
        .into_iter()
        .filter(|c| !hidden.iter().any(|h| h == c.key))
        .filter(|c| match c.id {
            ColumnId::Org => show_org && width >= NARROW,
            ColumnId::Reviews => width >= NARROW,
            ColumnId::Repo => width >= NARROWER,
            ColumnId::Margin
            | ColumnId::Status
            | ColumnId::Number
            | ColumnId::Title
            | ColumnId::Author
            | ColumnId::Comments
            | ColumnId::Age => true,
        })
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
