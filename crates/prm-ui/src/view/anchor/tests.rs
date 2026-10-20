use std::collections::HashMap;

use prm_domain::testing::key;

use super::*;

/// PR rows `keys` at display positions 0..n, with optional header rows first.
fn list(headers: usize, keys: &[PrKey]) -> DisplayList {
    let mut rows: Vec<DisplayRow> = (0..headers)
        .map(|g| DisplayRow::Group(u32::try_from(g).expect("small")))
        .collect();
    let mut index_of = HashMap::new();
    for (i, k) in keys.iter().enumerate() {
        index_of.insert(k.clone(), rows.len());
        rows.push(DisplayRow::Pr(u32::try_from(i).expect("small")));
    }
    DisplayList { rows, index_of }
}

#[test]
fn capture_prefers_the_selection_over_the_top_row() {
    let (a, b) = (key("o", 1, 1), key("o", 1, 2));
    let shown = list(0, &[a.clone(), b.clone()]);
    let snapshot = capture(&shown, Some(&b), Some(0), 24.0).expect("anchor");
    assert_eq!((snapshot.key, snapshot.display_index), (b, 1));
    let top = capture(&shown, None, Some(0), 24.0).expect("anchor");
    assert_eq!(top.key, a);
    assert_eq!(capture(&shown, None, None, 0.0), None);
}

#[test]
fn restore_shifts_the_offset_by_how_far_the_anchor_moved() {
    let (a, b) = (key("o", 1, 1), key("o", 1, 2));
    let snapshot =
        capture(&list(0, &[a.clone(), b.clone()]), Some(&b), None, 48.0).expect("anchor");
    let moved_down = list(2, &[a, b]);
    assert_eq!(
        restore(&snapshot, &moved_down, 24.0),
        AnchorResult::Offset(96.0)
    );
}

#[test]
fn restore_never_scrolls_above_the_top() {
    let (a, b) = (key("o", 1, 1), key("o", 1, 2));
    let snapshot = capture(&list(0, &[a.clone(), b.clone()]), Some(&b), None, 0.0).expect("anchor");
    assert_eq!(
        restore(&snapshot, &list(0, &[b, a]), 24.0),
        AnchorResult::Offset(0.0)
    );
}

#[test]
fn a_vanished_selection_selects_the_row_now_in_its_place() {
    let (a, b, c) = (key("o", 1, 1), key("o", 1, 2), key("o", 1, 3));
    let snapshot = capture(
        &list(0, &[a.clone(), b, c.clone()]),
        Some(&key("o", 1, 2)),
        None,
        0.0,
    )
    .expect("anchor");
    assert_eq!(
        restore(&snapshot, &list(0, &[a, c]), 24.0),
        AnchorResult::SelectNeighbor { index: 1 }
    );
    assert_eq!(restore(&snapshot, &list(0, &[]), 24.0), AnchorResult::None);
}
