//! The dashboard view model: rows precomputed once per data change, then filtered, sorted and
//! grouped by pure functions the UI calls when the query changes.

pub mod age;
pub mod group;
pub mod model;
pub mod query;
pub mod row;
pub mod sort;
pub mod summary;
pub mod text;

pub use age::{AgeBucket, AgeCache, AgeLabel, age_label};
pub use group::{GroupBy, GroupKey, GroupSpan};
pub use model::{ViewModel, build_view};
pub use query::{
    ParsedFilter, StatusFilter, ViewQuery, filtered_summary, parse_filter, row_matches,
};
pub use row::{PrRow, ReviewerChip, RowBuildInput, ThreadPreview, build_rows};
pub use sort::{Sort, SortKey, compare_rows};
pub use summary::{StatusSummary, TrayBadge, summarize, tray_badge, tray_tooltip};

#[cfg(test)]
mod tests;
