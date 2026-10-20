//! Column sorting with a stable, documented tie-break.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::view::row::PrRow;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Number,
    Title,
    Repo,
    Author,
    /// Creation time.
    Age,
    Activity,
    Comments,
    Org,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Status,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sort {
    pub key: SortKey,
    pub descending: bool,
}

/// Compares by the sort key; ties fall back to attention rank, then newest first, then key.
pub fn compare_rows(a: &PrRow, b: &PrRow, sort: Sort) -> Ordering {
    let primary = compare_key(a, b, sort.key);
    let primary = if sort.descending {
        primary.reverse()
    } else {
        primary
    };
    primary
        .then(a.status.attention_rank().cmp(&b.status.attention_rank()))
        .then(b.created_at.cmp(&a.created_at))
        .then(a.key.cmp(&b.key))
}

fn compare_key(a: &PrRow, b: &PrRow, key: SortKey) -> Ordering {
    match key {
        SortKey::Status => a.status.attention_rank().cmp(&b.status.attention_rank()),
        SortKey::Number => a.key.number.cmp(&b.key.number),
        SortKey::Title => a.title_key.cmp(&b.title_key),
        SortKey::Repo => a.repo_key.cmp(&b.repo_key),
        SortKey::Author => a.author_key.cmp(&b.author_key),
        // "Age ascending" means youngest first.
        SortKey::Age => b.created_at.cmp(&a.created_at),
        SortKey::Activity => b.last_activity.cmp(&a.last_activity),
        SortKey::Comments => b.active_threads.cmp(&a.active_threads),
        SortKey::Org => a.org_label.to_lowercase().cmp(&b.org_label.to_lowercase()),
    }
}
