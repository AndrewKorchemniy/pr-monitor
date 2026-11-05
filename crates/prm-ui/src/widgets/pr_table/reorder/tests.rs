use std::collections::BTreeMap;

use prm_domain::view::GroupBy;

use super::*;
use crate::view::columns::{
    ColumnLayout, DEFAULT_ORDER, Picks, Room, Width, placed, visible_columns,
};

/// The gap between columns, as the theme sets it (points).
const GAP: f32 = 8.0;
const HEADER: (f32, f32) = (10.0, 34.0);

/// The columns a wide window shows, in `order`, and their slots laid side by side from x = 0
/// the way `egui_extras` would, Title taking 300 points.
fn laid_out(order: &[ColumnUnit]) -> (Vec<ColumnSpec>, Vec<Rect>) {
    let widths = BTreeMap::new();
    let shown = ["required".to_owned()];
    let layout = ColumnLayout {
        group: GroupBy::None,
        show_org: true,
        picks: Picks {
            shown: &shown,
            ..Picks::default()
        },
        widths: &widths,
        order,
    };
    let room = Room {
        width: 1600.0,
        gap: GAP,
        margin: 8.0,
    };
    let columns = visible_columns(room, &layout);
    let mut x = 0.0;
    let slots = columns
        .iter()
        .map(|c| {
            let width = match c.width {
                Width::Exact => room.margin,
                Width::Fixed(w) | Width::Initial(w) => w,
                Width::Remainder => 300.0,
            };
            let slot = Rect::from_x_y_ranges(x..=x + width, HEADER.0..=HEADER.1);
            x += width + GAP;
            slot
        })
        .collect();
    (columns, slots)
}

fn cells_of(order: &[ColumnUnit]) -> (UnitCells, Rangef) {
    let (columns, slots) = laid_out(order);
    let right = slots.last().map_or(0.0, Rect::right) + 40.0;
    let span = Rangef::new(0.0, right);
    let rows = Rangef::new(HEADER.0, HEADER.1 + GAP / 2.0);
    (UnitCells::new(&columns, &slots, (span, rows)), span)
}

#[test]
fn the_cells_tile_the_header_with_no_gap_and_no_overlap() {
    let moved = placed(&DEFAULT_ORDER, ColumnUnit::Age, 1, |_| true);
    for (name, order) in [("default", DEFAULT_ORDER.to_vec()), ("moved", moved)] {
        let (cells, span) = cells_of(&order);
        let hits: Vec<Rect> = cells.iter().map(|c| c.hit).collect();
        assert_eq!(hits.len(), UNITS, "{name}");
        assert_eq!(hits.first().map(Rect::left), Some(span.min), "{name}");
        assert_eq!(hits.last().map(Rect::right), Some(span.max), "{name}");
        for pair in hits.windows(2) {
            assert_eq!(pair[0].right(), pair[1].left(), "{name}: {pair:?}");
        }
        for hit in &hits {
            assert_eq!(hit.y_range(), Rangef::new(HEADER.0, HEADER.1 + GAP / 2.0));
        }
    }
}

#[test]
fn the_cells_follow_the_order_and_the_pr_cell_holds_three_columns() {
    let order = placed(&DEFAULT_ORDER, ColumnUnit::Pr, 2, |_| true);
    let (cells, _) = cells_of(&order);
    let units: Vec<_> = cells.iter().map(|c| c.unit).collect();
    assert_eq!(units, order);
    let (columns, slots) = laid_out(&order);
    let own: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.id.unit() == Some(ColumnUnit::Pr))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(own.len(), 3);
    let first = slots[own[0]];
    let last = slots[own[2]];
    let pr = cells.get(ColumnUnit::Pr).map(|c| c.slot);
    assert_eq!(pr, Some(first.union(last)));
}

#[test]
fn the_margin_is_part_of_the_first_cell_whichever_unit_leads() {
    for lead in [ColumnUnit::Pr, ColumnUnit::Title] {
        let order = placed(&DEFAULT_ORDER, lead, 0, |_| true);
        let (cells, span) = cells_of(&order);
        let first = cells
            .iter()
            .next()
            .map(|c| (c.unit, c.hit.left(), c.slot.left()));
        let (unit, hit, slot) = first.unwrap_or((ColumnUnit::Org, f32::NAN, f32::NAN));
        assert_eq!((unit, hit), (lead, span.min));
        assert!(
            slot > hit,
            "the label starts after the margin ({slot} vs {hit})"
        );
    }
}

#[test]
fn a_cell_left_where_it_lies_follows_what_it_follows_now() {
    let (cells, _) = cells_of(&DEFAULT_ORDER);
    for (index, unit) in DEFAULT_ORDER.into_iter().enumerate() {
        let left = cells.get(unit).map_or(0.0, |c| c.slot.left());
        assert_eq!(cells.among(unit, left), index, "{unit:?}");
    }
    assert_eq!(cells.among(ColumnUnit::Title, -500.0), 0);
    assert_eq!(cells.among(ColumnUnit::Title, 9000.0), UNITS - 1);
}

#[test]
fn a_cell_passes_a_neighbour_once_its_leading_edge_is_past_the_middle() {
    let (cells, _) = cells_of(&DEFAULT_ORDER);
    let slot = |unit| cells.get(unit).map_or(Rect::NOTHING, |c| c.slot);
    let (title, repo, pr) = (
        slot(ColumnUnit::Title),
        slot(ColumnUnit::Repo),
        slot(ColumnUnit::Pr),
    );
    // Going right: Title's right edge against Repo's middle.
    let reach = repo.center().x - title.width();
    let at = |left| cells.among(ColumnUnit::Title, left);
    assert_eq!((at(reach - 1.0), at(reach + 1.0)), (1, 2));
    // Going left: Title's left edge against PR's middle.
    let before = pr.center().x;
    assert_eq!((at(before + 1.0), at(before - 1.0)), (1, 0));
}

/// Sweeps the pointer across the header, re-laying the cells out each step the way the table
/// does, and returns the place of the dragged unit after each step.
fn sweep(unit: ColumnUnit, grab: f32, xs: impl Iterator<Item = f32>) -> Vec<usize> {
    let mut order = DEFAULT_ORDER.to_vec();
    let mut places = Vec::new();
    for x in xs {
        let (cells, _) = cells_of(&order);
        let left = (x - grab).clamp(cells.reach(unit).min, cells.reach(unit).max);
        let among = cells.among(unit, left);
        if cells.position(unit) != Some(among) {
            order = placed(&order, unit, among, |_| true);
        }
        let (now, _) = cells_of(&order);
        places.push(now.position(unit).unwrap_or(usize::MAX));
    }
    places
}

/// Cells of unequal width must not swap back and forth under a pointer that holds still or
/// moves steadily: the place only ever grows going right and shrinks going left.
#[test]
fn a_swept_cell_never_flips_back_and_forth() {
    for unit in [ColumnUnit::Title, ColumnUnit::Pr, ColumnUnit::Reviews] {
        let right: Vec<f32> = (0..1700).step_by(7).map(|x| x as f32).collect();
        let places = sweep(unit, 20.0, right.iter().copied());
        assert!(
            places.windows(2).all(|w| w[0] <= w[1]),
            "{unit:?}: {places:?}"
        );
        assert_eq!(places.last(), Some(&(UNITS - 1)), "{unit:?}");
        let left = right.iter().rev().copied();
        let places = sweep(unit, 20.0, left);
        assert!(
            places.windows(2).all(|w| w[0] >= w[1]),
            "{unit:?}: {places:?}"
        );
        assert_eq!(places.last(), Some(&0), "{unit:?}");
    }
}

#[test]
fn a_dragged_cell_stays_inside_the_row() {
    let (cells, _) = cells_of(&DEFAULT_ORDER);
    let first = cells.iter().next().map(|c| c.slot.left());
    let last = cells.iter().last().map(|c| c.slot.right());
    let width = cells.get(ColumnUnit::Title).map_or(0.0, |c| c.slot.width());
    let reach = cells.reach(ColumnUnit::Title);
    assert_eq!(Some(reach.min), first);
    assert_eq!(Some(reach.max + width), last);
}

#[test]
fn the_slide_is_still_moving_until_every_cell_arrives() {
    let mut slide = Slide::default();
    assert!(!slide.moving());
    slide.set(ColumnUnit::Age, 12.0);
    assert!(slide.moving());
    assert_eq!(slide.of(ColumnUnit::Age), 12.0);
    assert_eq!(slide.of(ColumnUnit::Pr), 0.0);
    slide.set(ColumnUnit::Age, 0.1);
    assert!(!slide.moving());
}
