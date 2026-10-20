use super::*;

/// A table from (0, 100) to (800, 600) logical points with 24 pt rows at 1.5× scale.
fn geometry(scroll: f32) -> TableGeometry {
    TableGeometry::new([0.0, 100.0, 800.0, 600.0], scroll, 24.0, 1.5)
}

#[test]
fn geometry_is_in_physical_pixels() {
    let g = geometry(0.0);
    assert_eq!((g.x0, g.x1, g.top, g.bottom), (0, 1200, 150, 900));
    assert_eq!(g.pitch_milli, 36_000);
}

#[test]
fn points_map_to_row_bands_and_scrolling_shifts_them() {
    let g = geometry(0.0);
    assert_eq!(band_of(&g, 10, 150), Some(0));
    assert_eq!(band_of(&g, 10, 185), Some(0));
    assert_eq!(band_of(&g, 10, 186), Some(1));
    assert_eq!(band_of(&g, 10, 100), None, "above the table");
    assert_eq!(band_of(&g, 1300, 200), None, "right of the table");
    let scrolled = geometry(12.0);
    assert_eq!(band_of(&scrolled, 10, 150), Some(0));
    assert_eq!(band_of(&scrolled, 10, 168), Some(1));
}

#[test]
fn moves_within_one_row_are_dropped_until_the_row_changes() {
    let bands = HoverBands::default();
    bands.publish(Some(geometry(0.0)), true);
    assert!(bands.should_deliver(10, 160, 0));
    assert!(!bands.should_deliver(40, 170, 0));
    assert!(bands.should_deliver(40, 190, 0));
    assert!(!bands.should_deliver(41, 191, 0));
}

#[test]
fn buttons_outside_points_and_disabled_bands_always_deliver() {
    let bands = HoverBands::default();
    bands.publish(Some(geometry(0.0)), true);
    assert!(bands.should_deliver(10, 160, 0));
    assert!(bands.should_deliver(10, 161, 1), "a held button");
    assert!(bands.should_deliver(10, 50, 0), "outside");
    bands.publish(Some(geometry(0.0)), false);
    assert!(bands.should_deliver(10, 160, 0));
    assert!(bands.should_deliver(10, 160, 0));
    bands.publish(None, true);
    assert!(bands.should_deliver(10, 160, 0));
    assert!(bands.should_deliver(10, 160, 0));
}
