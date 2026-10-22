//! The visible dashboard: which rows, in what order, under which group headers.

use crate::view::{
    group::{GroupBy, GroupKey, GroupSpan},
    query::{ViewQuery, broad_matches, parse_filter, toggles_match},
    row::PrRow,
    sort::{Sort, compare_rows},
    summary::{StatusSummary, summarize},
};

/// Indices into the row slice, plus group spans over those indices.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ViewModel {
    pub indices: Vec<u32>,
    pub groups: Vec<GroupSpan>,
    /// Counts over the rows that pass the filter.
    pub counts: StatusSummary,
    /// Counts over the rows the text, organization and draft filters keep, whatever the status
    /// and changed-only filters: what the status bar's filter toggles count.
    pub broad_counts: StatusSummary,
}

/// Filter, then stable-sort by (group, column sort), then cut into group spans.
/// O(n log n); fast enough to rebuild on every keystroke for a thousand rows.
pub fn build_view(rows: &[PrRow], query: &ViewQuery, sort: Sort, group: GroupBy) -> ViewModel {
    let filter = parse_filter(&query.text);
    let broad: Vec<usize> = (0..rows.len())
        .filter(|i| broad_matches(&rows[*i], query, &filter))
        .collect();
    // Each visible row's group key is computed once, not once per comparison.
    let mut visible = keyed_matching_rows(rows, &broad, query, group);
    visible.sort_by(|(ka, a), (kb, b)| {
        ka.order(kb)
            .then_with(|| compare_rows(&rows[*a], &rows[*b], sort))
    });
    ViewModel {
        counts: summarize(visible.iter().map(|(_, i)| &rows[*i])),
        broad_counts: summarize(broad.iter().map(|i| &rows[*i])),
        indices: visible
            .iter()
            .map(|(_, i)| u32::try_from(*i).unwrap_or(u32::MAX))
            .collect(),
        groups: group_spans(rows, visible, group),
    }
}

/// Of the rows the broad filters kept (`broad`), those the status and changed-only toggles keep
/// too, each with its group key.
fn keyed_matching_rows(
    rows: &[PrRow],
    broad: &[usize],
    query: &ViewQuery,
    group: GroupBy,
) -> Vec<(GroupKey, usize)> {
    broad
        .iter()
        .copied()
        .filter(|i| toggles_match(&rows[*i], query))
        .map(|i| (GroupKey::of(&rows[i], group), i))
        .collect()
}

fn group_spans(rows: &[PrRow], visible: Vec<(GroupKey, usize)>, group: GroupBy) -> Vec<GroupSpan> {
    if group == GroupBy::None {
        return Vec::new();
    }
    let mut spans: Vec<GroupSpan> = Vec::new();
    for (position, (key, index)) in visible.into_iter().enumerate() {
        match spans.last_mut() {
            Some(span) if span.key == key => span.len += 1,
            Some(_) | None => spans.push(GroupSpan {
                label: key.label(&rows[index]),
                key,
                start: u32::try_from(position).unwrap_or(u32::MAX),
                len: 1,
            }),
        }
    }
    spans
}
