use super::*;

fn ids(columns: &[ColumnSpec]) -> Vec<ColumnId> {
    columns.iter().map(|c| c.id).collect()
}

#[test]
fn wide_windows_show_every_column_and_org_only_when_asked() {
    let wide = visible_columns(1400.0, true, &[]);
    assert_eq!(ids(&wide), ids(&COLUMNS));
    let no_org = visible_columns(1400.0, false, &[]);
    assert!(!ids(&no_org).contains(&ColumnId::Org));
}

#[test]
fn narrow_windows_drop_reviews_org_then_repo() {
    let narrow = ids(&visible_columns(NARROW - 1.0, true, &[]));
    assert!(!narrow.contains(&ColumnId::Reviews));
    assert!(!narrow.contains(&ColumnId::Org));
    assert!(narrow.contains(&ColumnId::Repo));
    let narrower = ids(&visible_columns(NARROWER - 1.0, true, &[]));
    assert!(!narrower.contains(&ColumnId::Repo));
    assert!(narrower.contains(&ColumnId::Title));
}

#[test]
fn hidden_columns_are_left_out_by_key() {
    let shown = ids(&visible_columns(1400.0, true, &["author".to_owned()]));
    assert!(!shown.contains(&ColumnId::Author));
}

#[test]
fn the_hash_changes_with_the_visible_set() {
    let all = visible_columns(1400.0, true, &[]);
    let fewer = visible_columns(NARROWER - 1.0, true, &[]);
    assert_ne!(columns_hash(&all), columns_hash(&fewer));
    assert_eq!(
        columns_hash(&all),
        columns_hash(&visible_columns(1500.0, true, &[]))
    );
}

#[test]
fn column_keys_are_unique() {
    let mut keys: Vec<_> = COLUMNS.iter().map(|c| c.key).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), COLUMNS.len());
}
