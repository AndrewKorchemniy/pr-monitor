//! The Corners setting: a click re-styles everything at once, with no restart.

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use prm_theme::Corners;

use crate::{
    copy,
    screens::settings::Page,
    testing::{
        sample_rows,
        screens::{Fixture, Shown, harness},
        state_with,
    },
};

fn settings_fixture() -> Fixture {
    let mut fixture = Fixture::new(Shown::Settings, state_with(sample_rows()));
    fixture.settings.page = Page::Appearance;
    fixture
}

/// What a user sees: the theme's corners and radii, and egui's window and menu rounding.
fn shown(h: &egui_kittest::Harness<'_, Fixture>) -> (Corners, [u8; 5], [u8; 2]) {
    let m = &h.state().theme.current(&h.ctx).tokens.metrics;
    let r = &m.radius;
    let visuals = h.ctx.global_style().visuals.clone();
    (
        m.corners,
        [r.xs, r.sm, r.md, r.lg, r.pill],
        [
            visuals.window_corner_radius.nw,
            visuals.menu_corner_radius.nw,
        ],
    )
}

#[test]
fn clicking_square_squares_the_whole_ui_and_rounded_brings_it_back() {
    let mut h = harness(settings_fixture());
    h.run();
    let (corners, radii, egui_radii) = shown(&h);
    assert_eq!(corners, Corners::Rounded);
    assert!(radii.iter().all(|&r| r > 0) && egui_radii.iter().all(|&r| r > 0));
    assert!(h.state().theme.chrome(&h.ctx).rounded);

    h.get_by_role_and_label(Role::RadioButton, copy::SQUARE)
        .click();
    h.run();
    assert_eq!(shown(&h), (Corners::Square, [0; 5], [0; 2]));
    assert!(!h.state().theme.chrome(&h.ctx).rounded);

    h.get_by_role_and_label(Role::RadioButton, copy::ROUNDED)
        .click();
    h.run();
    assert_eq!(shown(&h).0, Corners::Rounded);
    assert!(h.state().theme.chrome(&h.ctx).rounded);
}
