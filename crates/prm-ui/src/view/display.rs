//! The table's display list: group header rows interleaved with PR rows, honoring collapsed
//! groups.

use std::collections::{BTreeSet, HashMap};

use prm_domain::{
    PrKey, PrStatus,
    view::{GroupBy, GroupKey, PrRow, ViewModel},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum DisplayRow {
    /// Index into `ViewModel::groups`.
    Group(u32),
    /// Index into the row slice.
    Pr(u32),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DisplayList {
    pub rows: Vec<DisplayRow>,
    /// Where each visible PR sits in `rows`.
    pub index_of: HashMap<PrKey, usize>,
}

impl DisplayList {
    /// The row slice index at a display position, when it is a PR row.
    pub(crate) fn pr_at(&self, display: usize) -> Option<usize> {
        match self.rows.get(display)? {
            DisplayRow::Pr(row) => usize::try_from(*row).ok(),
            DisplayRow::Group(_) => None,
        }
    }
}

const fn status_id(status: PrStatus) -> &'static str {
    match status {
        PrStatus::NeedsReview => "status:needs_review",
        PrStatus::ActiveComments => "status:active_comments",
        PrStatus::Reviewed => "status:reviewed",
    }
}

/// The persisted id of a group (for `UiState.collapsed_groups`).
pub(crate) fn group_id(key: &GroupKey) -> String {
    match key {
        GroupKey::None => String::new(),
        GroupKey::Status(status) => status_id(*status).to_owned(),
        GroupKey::Text(text) => format!("text:{text}"),
        GroupKey::Org(org) => format!("org:{org}"),
    }
}

/// Group headers only when grouping; a collapsed group keeps its header and hides its rows.
pub(crate) fn build_display(
    rows: &[PrRow],
    model: &ViewModel,
    group: GroupBy,
    collapsed: &BTreeSet<String>,
) -> DisplayList {
    let mut list = DisplayList::default();
    let push_pr = |list: &mut DisplayList, view_index: usize| {
        let Some(&row) = model.indices.get(view_index) else {
            return;
        };
        let Some(pr) = usize::try_from(row).ok().and_then(|r| rows.get(r)) else {
            return;
        };
        list.index_of.insert(pr.key.clone(), list.rows.len());
        list.rows.push(DisplayRow::Pr(row));
    };
    if group == GroupBy::None {
        for view_index in 0..model.indices.len() {
            push_pr(&mut list, view_index);
        }
        return list;
    }
    for (span_index, span) in model.groups.iter().enumerate() {
        list.rows.push(DisplayRow::Group(
            u32::try_from(span_index).unwrap_or(u32::MAX),
        ));
        if collapsed.contains(&group_id(&span.key)) {
            continue;
        }
        let start = usize::try_from(span.start).unwrap_or(0);
        let len = usize::try_from(span.len).unwrap_or(0);
        for view_index in start..start + len {
            push_pr(&mut list, view_index);
        }
    }
    list
}

#[cfg(test)]
mod tests;
