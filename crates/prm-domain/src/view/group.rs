//! Grouping rows into labeled spans ("whose move it is" by default).

use std::{cmp::Ordering, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{classify::PrStatus, ids::OrgId, view::row::PrRow};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    None,
    Repo,
    Author,
    Org,
    Project,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Status,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupKey {
    None,
    Status(PrStatus),
    Text(Arc<str>),
    Org(OrgId),
}

impl GroupKey {
    pub fn of(row: &PrRow, by: GroupBy) -> Self {
        match by {
            GroupBy::None => Self::None,
            GroupBy::Status => Self::Status(row.status),
            GroupBy::Repo => Self::Text(row.repo_name.clone()),
            GroupBy::Author => Self::Text(row.author.display_name()),
            GroupBy::Project => Self::Text(row.project_name.clone()),
            GroupBy::Org => Self::Org(row.key.org.clone()),
        }
    }

    /// Status groups by attention rank; text and org groups alphabetically (case-insensitive).
    pub fn order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Status(a), Self::Status(b)) => a.attention_rank().cmp(&b.attention_rank()),
            (Self::Text(a), Self::Text(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
            (Self::Org(a), Self::Org(b)) => a.cmp(b),
            // why: one GroupBy only ever produces one kind of key, so mixed pairs never occur.
            _ => Ordering::Equal,
        }
    }

    pub fn label(&self, row: &PrRow) -> Arc<str> {
        match self {
            Self::None => Arc::from(""),
            Self::Status(status) => Arc::from(status.label()),
            Self::Text(text) => text.clone(),
            Self::Org(_) => row.org_label.clone(),
        }
    }
}

/// A contiguous run of rows (indices into the view) sharing a group key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSpan {
    pub key: GroupKey,
    pub label: Arc<str>,
    pub start: u32,
    pub len: u32,
}
