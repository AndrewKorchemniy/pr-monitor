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
fn hidden_never_drops_status_title_or_the_margin() {
    let hidden = ["", "status", "title", "author", "flags"].map(str::to_owned);
    let shown = ids(&visible_columns(1400.0, true, &hidden));
    for kept in [ColumnId::Margin, ColumnId::Status, ColumnId::Title] {
        assert!(shown.contains(&kept), "{kept:?} must stay");
    }
    for gone in [ColumnId::Author, ColumnId::Flags] {
        assert!(!shown.contains(&gone), "{gone:?} was hidden");
    }
}

#[test]
fn hidden_columns_and_narrow_windows_combine() {
    let shown = ids(&visible_columns(
        NARROWER - 1.0,
        true,
        &["author".to_owned()],
    ));
    assert_eq!(
        shown,
        [
            ColumnId::Margin,
            ColumnId::Status,
            ColumnId::Number,
            ColumnId::Flags,
            ColumnId::Title,
            ColumnId::Comments,
            ColumnId::Age,
        ]
    );
}

#[test]
fn the_column_menu_lists_everything_but_the_margin_and_org() {
    let listed: Vec<_> = listed().map(|c| (c.id, c.visibility())).collect();
    let (locked, optional) = (Visibility::Locked, Visibility::Optional);
    assert_eq!(
        listed,
        [
            (ColumnId::Status, locked),
            (ColumnId::Number, optional),
            (ColumnId::Flags, optional),
            (ColumnId::Title, locked),
            (ColumnId::Repo, optional),
            (ColumnId::Author, optional),
            (ColumnId::Reviews, optional),
            (ColumnId::Comments, optional),
            (ColumnId::Age, optional),
        ]
    );
}

#[test]
fn flags_come_right_before_the_title() {
    let at = |id| COLUMNS.iter().position(|c| c.id == id);
    assert_eq!(at(ColumnId::Flags).map(|i| i + 1), at(ColumnId::Title));
}

#[test]
fn only_columns_with_an_initial_width_resize() {
    let fixed = [ColumnId::Margin, ColumnId::Flags, ColumnId::Title];
    for column in COLUMNS {
        assert_eq!(
            column.is_resizable(),
            !fixed.contains(&column.id),
            "{:?}",
            column.id
        );
    }
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
