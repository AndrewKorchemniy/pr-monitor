use super::*;

/// The palette listed only its first rows, so in a short window the highlight could move to a
/// row that was not drawn.
#[test]
fn visible_rows_follow_the_highlight() {
    let cases = [
        ((0, 20, 5), 0..5),
        ((4, 20, 5), 0..5),
        ((5, 20, 5), 1..6),
        ((19, 20, 5), 15..20),
        ((0, 3, 5), 0..3),
        ((2, 3, 5), 0..3),
        ((0, 0, 5), 0..0),
    ];
    for ((selected, count, rows), expected) in cases {
        let shown = visible(selected, count, rows);
        assert_eq!(shown, expected, "row {selected} of {count}, {rows} shown");
        assert!(count == 0 || shown.contains(&selected), "{shown:?}");
    }
}

#[test]
fn rows_that_fit_shrink_with_the_window_but_keep_a_few() {
    let theme = crate::theme::fallback();
    assert_eq!(rows_that_fit(&theme, 2000.0), MAX_ROWS);
    assert_eq!(rows_that_fit(&theme, 100.0), MIN_ROWS);
    let pitch = theme.control_height() + theme.tokens.metrics.space.xxs;
    let short = rows_that_fit(&theme, 530.0);
    assert!((MIN_ROWS..MAX_ROWS).contains(&short), "{short}");
    #[expect(clippy::cast_precision_loss, reason = "a dozen rows")]
    let listed = short as f32 * pitch;
    assert!(listed < 530.0, "{short} rows take {listed} pt");
}
