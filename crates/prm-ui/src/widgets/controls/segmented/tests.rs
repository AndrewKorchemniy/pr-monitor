use super::*;

const PADS: (f32, f32) = (16.0, 6.0);

#[test]
fn arrange_keeps_natural_widths_when_there_is_room() {
    let texts = [62.0, 45.0, 40.0];
    assert_eq!(
        arrange(&texts, 600.0, false, PADS),
        Arrangement::Row(vec![94.0, 77.0, 72.0])
    );
}

/// "Automatic | OpenGL | Direct3D" kept its natural width and ran out of a narrow row.
#[test]
fn arrange_shrinks_padding_then_stacks_when_the_row_is_narrow() {
    let texts = [62.0, 45.0, 40.0];
    let Arrangement::Row(widths) = arrange(&texts, 201.0, false, PADS) else {
        panic!("still fits in a row");
    };
    let used: f32 = widths.iter().sum();
    assert!((used - 201.0).abs() < 0.01, "{widths:?}");
    assert_eq!(
        arrange(&texts, 160.0, false, PADS),
        Arrangement::Column(94.0),
        "less than 12 pt around each text"
    );
    assert_eq!(
        arrange(&texts, 80.0, false, PADS),
        Arrangement::Column(80.0)
    );
}

#[test]
fn arrange_fills_a_justified_row_in_equal_widths() {
    let texts = [62.0, 45.0, 40.0];
    assert_eq!(
        arrange(&texts, 312.0, true, PADS),
        Arrangement::Row(vec![104.0; 3])
    );
    // A long first option: equal widths would clip it, so the padding is shared instead.
    let Arrangement::Row(widths) = arrange(&[150.0, 20.0, 20.0], 240.0, true, PADS) else {
        panic!("fits in a row");
    };
    assert!(widths[0] > widths[1], "{widths:?}");
    assert_eq!(
        arrange(&texts, 150.0, true, PADS),
        Arrangement::Column(150.0)
    );
}

#[test]
fn option_rects_tile_a_row_and_a_column() {
    let row = option_rects(&Arrangement::Row(vec![10.0, 20.0]), 2, 30.0);
    assert_eq!(
        row,
        [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 30.0)),
            Rect::from_min_size(pos2(10.0, 0.0), vec2(20.0, 30.0)),
        ]
    );
    let column = option_rects(&Arrangement::Column(50.0), 2, 30.0);
    assert_eq!(
        column[1],
        Rect::from_min_size(pos2(0.0, 30.0), vec2(50.0, 30.0))
    );
}
