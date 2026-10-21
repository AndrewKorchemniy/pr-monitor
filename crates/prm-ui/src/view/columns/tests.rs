use super::*;

/// The theme's defaults: 8 pt between columns, an 8 pt change-bar margin.
fn room(width: f32) -> Room {
    Room {
        width,
        gap: 8.0,
        margin: 8.0,
    }
}

fn layout<'a>(
    group: GroupBy,
    hidden: &'a [String],
    widths: &'a BTreeMap<String, f32>,
) -> ColumnLayout<'a> {
    ColumnLayout {
        group,
        show_org: true,
        hidden,
        widths,
    }
}

fn ids(columns: &[ColumnSpec]) -> Vec<ColumnId> {
    columns.iter().map(|c| c.id).collect()
}

fn shown(width: f32, group: GroupBy, hidden: &[String]) -> Vec<ColumnId> {
    ids(&visible_columns(
        room(width),
        &layout(group, hidden, &BTreeMap::new()),
    ))
}

#[test]
fn wide_tables_show_every_column_and_org_only_when_asked() {
    let none = BTreeMap::new();
    let wide = visible_columns(room(1400.0), &layout(GroupBy::None, &[], &none));
    assert_eq!(ids(&wide), ids(&COLUMNS));
    let no_org = ColumnLayout {
        show_org: false,
        ..layout(GroupBy::None, &[], &none)
    };
    assert!(!ids(&visible_columns(room(1400.0), &no_org)).contains(&ColumnId::Org));
}

/// Adding the Flags column made the narrowest layout 50 pt wider while fixed width breakpoints
/// stayed put, so between them the table ran past its right edge and Age was cut off.
#[test]
fn visible_columns_always_fit_until_only_the_core_is_left() {
    let none = BTreeMap::new();
    for group in [GroupBy::Status, GroupBy::None] {
        for step in 0..=120 {
            let width = 400.0 + 10.0 * step as f32;
            let wants = layout(group, &[], &none);
            let columns = visible_columns(room(width), &wants);
            let needed = wants.needed(&columns, room(width));
            let core = SQUEEZE_ORDER.iter().all(|id| !ids(&columns).contains(id));
            assert!(
                needed <= width || core,
                "{group:?} at {width} pt needs {needed} pt"
            );
        }
    }
}

#[test]
fn narrow_tables_drop_columns_in_squeeze_order() {
    let mut dropped = Vec::new();
    for step in (0..=100).rev() {
        let width = 400.0 + 10.0 * step as f32;
        let visible = shown(width, GroupBy::Status, &[]);
        let gone: Vec<_> = SQUEEZE_ORDER
            .into_iter()
            .filter(|id| !visible.contains(id))
            .collect();
        assert_eq!(gone, SQUEEZE_ORDER[..gone.len()], "at {width} pt");
        assert!(
            gone.len() >= dropped.len(),
            "a column came back at {width} pt"
        );
        dropped = gone;
        for kept in [ColumnId::Status, ColumnId::Title, ColumnId::Age] {
            assert!(visible.contains(&kept), "{kept:?} at {width} pt");
        }
    }
    assert_eq!(dropped, SQUEEZE_ORDER);
}

#[test]
fn hiding_a_column_leaves_room_for_others() {
    let squeezed = shown(800.0, GroupBy::Status, &[]);
    assert!(!squeezed.contains(&ColumnId::Reviews));
    assert!(!squeezed.contains(&ColumnId::Repo));
    let roomier = shown(800.0, GroupBy::Status, &["author".to_owned()]);
    for back in [ColumnId::Reviews, ColumnId::Repo] {
        assert!(roomier.contains(&back), "{back:?}");
    }
    assert!(!roomier.contains(&ColumnId::Author));
}

#[test]
fn dragged_widths_count_toward_the_room_needed() {
    let hidden: [String; 0] = [];
    let default = BTreeMap::new();
    let wide_repo = BTreeMap::from([("repo".to_owned(), 300.0)]);
    let fits = visible_columns(room(930.0), &layout(GroupBy::Status, &hidden, &default));
    assert!(ids(&fits).contains(&ColumnId::Reviews));
    let tighter = visible_columns(room(930.0), &layout(GroupBy::Status, &hidden, &wide_repo));
    assert!(!ids(&tighter).contains(&ColumnId::Reviews));
}

#[test]
fn hidden_columns_are_left_out_by_key() {
    let visible = shown(1400.0, GroupBy::None, &["author".to_owned()]);
    assert!(!visible.contains(&ColumnId::Author));
}

#[test]
fn hidden_never_drops_status_title_or_the_margin() {
    let hidden = ["", "status", "title", "author", "flags"].map(str::to_owned);
    let visible = shown(1400.0, GroupBy::None, &hidden);
    for kept in [ColumnId::Margin, ColumnId::Status, ColumnId::Title] {
        assert!(visible.contains(&kept), "{kept:?} must stay");
    }
    for gone in [ColumnId::Author, ColumnId::Flags] {
        assert!(!visible.contains(&gone), "{gone:?} was hidden");
    }
}

#[test]
fn hidden_columns_and_narrow_tables_combine() {
    let visible = shown(600.0, GroupBy::Status, &["author".to_owned()]);
    assert_eq!(
        visible,
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
fn grouped_by_status_the_status_column_is_its_glyph() {
    let none = BTreeMap::new();
    let columns = visible_columns(room(1400.0), &layout(GroupBy::Status, &[], &none));
    let status = columns.iter().find(|c| c.id == ColumnId::Status);
    assert_eq!(
        status.map(|c| (c.header, c.width, c.sort)),
        Some(("", Width::Fixed(STATUS_GLYPH), None))
    );
    let number = columns.iter().find(|c| c.id == ColumnId::Number);
    assert_eq!(number.map(|c| c.key), Some("number_left"));
}

#[test]
fn squeezed_lists_chosen_columns_the_table_left_out() {
    let hidden = ["author".to_owned()];
    let none = BTreeMap::new();
    let visible = visible_columns(room(600.0), &layout(GroupBy::Status, &hidden, &none));
    let left_out: Vec<_> = squeezed(&visible, &hidden).collect();
    assert_eq!(left_out, [ColumnId::Repo, ColumnId::Reviews]);
    let choices = Choices {
        hidden: &hidden,
        squeezed: &left_out,
    };
    let check = |id| column(id).map(|c| choices.check(&c));
    assert_eq!(check(ColumnId::Repo), Some(ColumnCheck::Squeezed));
    assert_eq!(check(ColumnId::Author), Some(ColumnCheck::Off));
    assert_eq!(check(ColumnId::Age), Some(ColumnCheck::On));
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
    let none = BTreeMap::new();
    let at = |width| visible_columns(room(width), &layout(GroupBy::None, &[], &none));
    assert_ne!(columns_hash(&at(1400.0)), columns_hash(&at(700.0)));
    assert_eq!(columns_hash(&at(1400.0)), columns_hash(&at(1500.0)));
}

#[test]
fn column_keys_are_unique() {
    let mut keys: Vec<_> = COLUMNS.iter().map(|c| c.key).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), COLUMNS.len());
}
