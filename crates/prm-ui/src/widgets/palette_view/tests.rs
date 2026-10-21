use super::*;

#[test]
fn rows_that_fit_shrink_with_the_window_but_keep_a_few() {
    let theme = crate::theme::fallback();
    assert_eq!(rows_that_fit(&theme, 2000.0), MAX_ROWS);
    assert_eq!(rows_that_fit(&theme, 100.0), MIN_ROWS);
    let short = rows_that_fit(&theme, 530.0);
    assert!((MIN_ROWS..MAX_ROWS).contains(&short), "{short}");
    let listed = list_height(&theme, short, short);
    assert!(listed < 530.0, "{short} rows take {listed} pt");
}

/// The list box is exactly as tall as its rows when they all fit; when more are below, half of
/// the next one shows, so the list reads as scrollable.
#[test]
fn the_list_shows_half_a_row_when_more_are_below() {
    let theme = crate::theme::fallback();
    let row = theme.control_height();
    let gap = theme.tokens.metrics.space.xxs;
    assert_eq!(list_height(&theme, 1, 1), row);
    assert_eq!(list_height(&theme, 3, 3), 3.0 * row + 2.0 * gap);
    let peek = list_height(&theme, 3, 10);
    assert!((peek - (2.5 * (row + gap) - gap)).abs() < 0.01, "{peek}");
}
