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
