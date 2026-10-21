use super::*;

/// The sidebar folds away exactly where keeping it would make the page's rows stack.
#[test]
fn the_sidebar_folds_into_a_page_menu_below_720_points() {
    assert_eq!(SIDEBAR_FROM, 720.0);
    assert_eq!(f32::from(PAGE_PAD), PAGE_INSET);
    let cases = [
        (344.0, Nav::Menu),
        (656.0, Nav::Menu),
        (719.0, Nav::Menu),
        (720.0, Nav::Sidebar),
        (1040.0, Nav::Sidebar),
    ];
    for (width, nav) in cases {
        assert_eq!(Nav::for_width(width), nav, "{width} pt");
    }
    let page = SIDEBAR_FROM - NAV_WIDTH - 2.0 * PAGE_INSET;
    assert!(
        !fit::row_stacks(page),
        "rows beside the sidebar keep their controls inline"
    );
}
