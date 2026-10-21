use std::collections::BTreeSet;

use prm_domain::view::{Sort, ViewQuery, build_view};

use super::*;
use crate::testing::sample_rows;

fn display(group: GroupBy, collapsed: &BTreeSet<String>) -> (DisplayList, ViewModel) {
    let rows = sample_rows();
    let model = build_view(&rows, &ViewQuery::default(), Sort::default(), group);
    (build_display(&rows, &model, group, collapsed), model)
}

#[test]
fn without_grouping_every_row_is_a_pr() {
    let (list, _) = display(GroupBy::None, &BTreeSet::new());
    assert_eq!(list.rows.len(), 4);
    assert!(list.rows.iter().all(|r| matches!(r, DisplayRow::Pr(_))));
    assert_eq!(list.index_of.len(), 4);
}

#[test]
fn grouping_puts_a_header_before_each_group() {
    let (list, model) = display(GroupBy::Status, &BTreeSet::new());
    assert_eq!(list.rows.len(), 4 + model.groups.len());
    assert!(matches!(list.rows[0], DisplayRow::Group(0)));
    assert_eq!(list.pr_at(0), None);
    assert!(list.pr_at(1).is_some());
    for (key, at) in &list.index_of {
        let row = list.pr_at(*at).expect("index_of points at PR rows");
        assert_eq!(&sample_rows()[row].key, key);
    }
}

#[test]
fn a_collapsed_group_keeps_its_header_and_hides_its_rows() {
    let (_, model) = display(GroupBy::Status, &BTreeSet::new());
    let first = group_id(&model.groups[0].key);
    let hidden = model.groups[0].len as usize;
    let (list, _) = display(GroupBy::Status, &BTreeSet::from([first]));
    assert_eq!(list.rows.len(), 4 + model.groups.len() - hidden);
    assert!(matches!(list.rows[0], DisplayRow::Group(0)));
    assert!(matches!(list.rows[1], DisplayRow::Group(1)));
}

#[test]
fn group_ids_are_stable_strings() {
    assert_eq!(
        group_id(&GroupKey::Status(PrStatus::NeedsReview)),
        "status:needs_review"
    );
    assert_eq!(
        group_id(&GroupKey::Text(std::sync::Arc::from("api"))),
        "text:api"
    );
    assert_eq!(group_id(&GroupKey::None), "");
}

#[test]
fn pitch_finds_tops_and_rows_when_cards_are_taller_than_headers() {
    let rows = [DisplayRow::Group(0), DisplayRow::Pr(0), DisplayRow::Pr(1)];
    let pitch = Pitch {
        pr: 78.0,
        group: 36.0,
    };
    let tops: Vec<f32> = (0..=3).map(|i| pitch.top_of(&rows, i)).collect();
    assert_eq!(tops, [0.0, 36.0, 114.0, 192.0]);
    let cases = [
        (0.0, 0),
        (35.9, 0),
        (36.0, 1),
        (113.0, 1),
        (114.0, 2),
        (900.0, 2),
    ];
    for (y, index) in cases {
        assert_eq!(pitch.index_at(&rows, y), index, "at {y}");
    }
    assert_eq!(Pitch::uniform(36.0).index_at(&rows, 80.0), 2);
    assert_eq!(pitch.index_at(&[], 10.0), 0);
}

#[test]
fn whole_prs_counts_the_cards_wholly_in_view() {
    let rows = [
        DisplayRow::Group(0),
        DisplayRow::Pr(0),
        DisplayRow::Pr(1),
        DisplayRow::Group(1),
        DisplayRow::Pr(2),
    ];
    let pitch = Pitch {
        pr: 78.0,
        group: 36.0,
    };
    let cases = [
        ((0.0, 192.0), 2),
        ((0.0, 191.0), 1),
        ((36.0, 300.0), 3),
        ((40.0, 300.0), 2),
        ((0.0, 10.0), 0),
    ];
    for (view, count) in cases {
        assert_eq!(pitch.whole_prs(&rows, view), count, "{view:?}");
    }
}
