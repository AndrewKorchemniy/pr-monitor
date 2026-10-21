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

/// The width a column takes in the table this frame, Title at its minimum (points).
fn width_in(column: &ColumnSpec, room: Room) -> f32 {
    match column.width {
        Width::Exact => room.margin,
        Width::Fixed(width) | Width::Initial(width) => width,
        Width::Remainder => column.min,
    }
}

/// The room `columns` take side by side, Title at its minimum (points).
fn taken(columns: &[ColumnSpec], room: Room) -> f32 {
    let gaps = room.gap * (columns.len() - 1) as f32;
    columns.iter().map(|c| width_in(c, room)).sum::<f32>() + gaps
}

/// The least room the core takes, each column at its minimum (points).
fn core_minimum(group: GroupBy, room: Room) -> f32 {
    let none = BTreeMap::new();
    let wide = visible_columns(
        Room {
            width: 4000.0,
            ..room
        },
        &layout(group, &[], &none),
    );
    let core: Vec<ColumnSpec> = wide
        .into_iter()
        .filter(|c| !SQUEEZE_ORDER.contains(&c.id))
        .map(|c| match c.width {
            Width::Initial(_) => ColumnSpec {
                width: Width::Initial(c.min),
                ..c
            },
            Width::Exact | Width::Fixed(_) | Width::Remainder => c,
        })
        .collect();
    taken(&core, room)
}

/// Adding the Flags column made the narrowest layout 50 pt wider while fixed width breakpoints
/// stayed put, so between them the table ran past its right edge and Age was cut off.
#[test]
fn visible_columns_always_fit_while_the_core_does() {
    let none = BTreeMap::new();
    for group in [GroupBy::Status, GroupBy::None] {
        for step in 0..=120 {
            let width = 400.0 + 10.0 * step as f32;
            let columns = visible_columns(room(width), &layout(group, &[], &none));
            let taken = taken(&columns, room(width));
            let core = core_minimum(group, room(width));
            assert!(
                taken <= width || core > width,
                "{group:?} at {width} pt takes {taken} pt"
            );
        }
    }
}

/// A width saved from a wide window made the table wider than a narrower one: saved widths
/// never shrank, so the rows ran past the window's right edge.
#[test]
fn large_saved_widths_never_make_the_table_wider_than_the_room() {
    let huge: BTreeMap<String, f32> = COLUMNS
        .iter()
        .filter(|c| c.is_resizable())
        .map(|c| (c.key.to_owned(), 900.0))
        .chain([("number_left".to_owned(), 900.0)])
        .collect();
    let before = huge.clone();
    for group in [GroupBy::Status, GroupBy::None] {
        for step in 0..=160 {
            let width = 400.0 + 10.0 * step as f32;
            let columns = visible_columns(room(width), &layout(group, &[], &huge));
            let taken = taken(&columns, room(width));
            let core = core_minimum(group, room(width));
            assert!(
                taken <= width || core > width,
                "{group:?} at {width} pt takes {taken} pt: {columns:?}"
            );
        }
    }
    assert_eq!(huge, before, "the saved widths stay the user's");
}

/// Columns give back the same share of their dragged room, and only what the window lacks.
#[test]
fn dragged_widths_shrink_by_the_same_share_before_a_column_drops() {
    let dragged = BTreeMap::from([("repo".to_owned(), 360.0), ("author".to_owned(), 250.0)]);
    let at = |width: f32| visible_columns(room(width), &layout(GroupBy::Status, &[], &dragged));
    let width_of = |columns: &[ColumnSpec], id| {
        columns
            .iter()
            .find(|c| c.id == id)
            .map(|c| width_in(c, room(0.0)))
    };
    let roomy = at(1600.0);
    assert_eq!(width_of(&roomy, ColumnId::Repo), Some(360.0));
    let squeezed = at(1200.0);
    assert_eq!(ids(&squeezed), ids(&roomy), "nothing drops yet");
    let repo = width_of(&squeezed, ColumnId::Repo).unwrap_or_default();
    let author = width_of(&squeezed, ColumnId::Author).unwrap_or_default();
    assert!(repo < 360.0 && author < 250.0, "{repo} {author}");
    let (repo_share, author_share) = ((360.0 - repo) / 200.0, (250.0 - author) / 100.0);
    assert!(
        (repo_share - author_share).abs() < 0.02,
        "{repo_share} {author_share}"
    );
    assert!(taken(&squeezed, room(1200.0)) <= 1200.0);
    assert!(
        taken(&squeezed, room(1200.0)) > 1190.0,
        "uses the room it has"
    );
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
fn dragged_widths_give_way_before_a_column_drops() {
    let hidden: [String; 0] = [];
    let default = BTreeMap::new();
    let wide_repo = BTreeMap::from([("repo".to_owned(), 300.0)]);
    let fits = visible_columns(room(930.0), &layout(GroupBy::Status, &hidden, &default));
    assert!(ids(&fits).contains(&ColumnId::Reviews));
    let tighter = visible_columns(room(930.0), &layout(GroupBy::Status, &hidden, &wide_repo));
    assert_eq!(ids(&tighter), ids(&fits));
    let repo = tighter.iter().find(|c| c.id == ColumnId::Repo);
    assert!(
        repo.is_some_and(|c| matches!(c.width, Width::Initial(w) if (160.0..300.0).contains(&w))),
        "{repo:?}"
    );
}

/// Below the core and Comments at their defaults, a table would show only the title and its
/// status, so the dashboard lists cards.
#[test]
fn a_table_needs_room_for_its_core_and_comments() {
    // Margin, Status, PR, Flags, Title (minimum), Comments, Age and six gaps.
    let minimum = 8.0 + 140.0 + 64.0 + FLAGS_WIDTH + 200.0 + 76.0 + 56.0 + 6.0 * 8.0;
    assert!(fits_table(room(minimum)));
    assert!(!fits_table(room(minimum - 1.0)));
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
