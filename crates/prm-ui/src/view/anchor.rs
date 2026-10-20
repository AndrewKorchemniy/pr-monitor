//! Keeping the user's place when data refreshes: the selected PR (or the top visible one) stays
//! at the same screen position. Selection is a key, never an index.

use prm_domain::PrKey;

use crate::view::display::{DisplayList, DisplayRow};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AnchorSnapshot {
    pub key: PrKey,
    pub display_index: usize,
    pub offset_y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnchorResult {
    /// Scroll so the anchor stays where it was.
    Offset(f32),
    /// The selected PR is gone: select this display row, keep the scroll offset.
    SelectNeighbor {
        index: usize,
    },
    None,
}

/// Prefer the selected key; else the top visible PR row.
pub(crate) fn capture(
    list: &DisplayList,
    selected: Option<&PrKey>,
    top_visible: Option<usize>,
    offset_y: f32,
) -> Option<AnchorSnapshot> {
    let from_selection = selected.and_then(|key| {
        let index = *list.index_of.get(key)?;
        Some((key.clone(), index))
    });
    let from_top = || {
        let start = top_visible?;
        (start..list.rows.len()).find_map(|i| {
            let key = list.index_of.iter().find(|(_, at)| **at == i)?.0;
            Some((key.clone(), i))
        })
    };
    let (key, display_index) = from_selection.or_else(from_top)?;
    Some(AnchorSnapshot {
        key,
        display_index,
        offset_y,
    })
}

/// `offset' = max(0, offset + (new − old) · pitch)` keeps the anchor at the same screen y.
pub(crate) fn restore(snapshot: &AnchorSnapshot, list: &DisplayList, pitch: f32) -> AnchorResult {
    if let Some(&new_index) = list.index_of.get(&snapshot.key) {
        let moved = row_delta(new_index, snapshot.display_index);
        return AnchorResult::Offset((snapshot.offset_y + moved * pitch).max(0.0));
    }
    if list.index_of.is_empty() {
        return AnchorResult::None;
    }
    let start = snapshot
        .display_index
        .min(list.rows.len().saturating_sub(1));
    let is_pr = |i: &usize| matches!(list.rows.get(*i), Some(DisplayRow::Pr(_)));
    let forward = (start..list.rows.len()).find(is_pr);
    let backward = || (0..start).rev().find(is_pr);
    forward
        .or_else(backward)
        .map_or(AnchorResult::None, |index| AnchorResult::SelectNeighbor {
            index,
        })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "row counts are far below f32 precision"
)]
fn row_delta(new: usize, old: usize) -> f32 {
    new as f32 - old as f32
}

#[cfg(test)]
mod tests;
