use super::*;

/// The checklist was always three wide (512 pt), so at the smallest window its third column
/// ran past the page's right edge.
#[test]
fn checks_per_line_fit_the_page_width() {
    let gap = 16.0;
    let cases = [
        (600.0, 3),
        (512.0, 3),
        (511.0, 2),
        (384.0, 2),
        (336.0, 2),
        (335.0, 1),
        (100.0, 1),
    ];
    for (width, expected) in cases {
        let per_line = checks_per_line(width, gap);
        assert_eq!(per_line, expected, "{width} pt");
        let used = per_line as f32 * (COLUMN_CHECK_WIDTH + gap) - gap;
        assert!(per_line == 1 || used <= width, "{width} pt: {used} pt used");
    }
}
