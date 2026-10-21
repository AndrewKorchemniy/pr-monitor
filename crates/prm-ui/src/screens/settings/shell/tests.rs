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

/// A taller window made settings narrower, so the sidebar folded away as the window grew.
#[test]
fn the_window_height_does_not_change_the_navigation() {
    let chrome = vec2(2.0, 2.0);
    for width in [600.0, 720.0, 760.0, 780.0, 800.0, 1280.0] {
        let nav = |height: f32| Nav::for_width(inner_size(vec2(width, height), chrome).x);
        let short = nav(420.0);
        for height in [560.0, 600.0, 640.0, 740.0, 800.0, 1200.0] {
            assert_eq!(nav(height), short, "{width} by {height} pt");
        }
    }
    let nav = |width: f32| Nav::for_width(inner_size(vec2(width, 800.0), chrome).x);
    assert_eq!(nav(760.0), Nav::Menu);
    assert_eq!(nav(790.0), Nav::Sidebar);
}
