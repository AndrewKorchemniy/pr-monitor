//! The Striped table rows setting: a click tints or clears every other row of the table and the
//! cards at once, with no restart. Checked on the shapes the frame emits, so no GPU is needed.

use eframe::egui::{self, Shape, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable as _};

use crate::{
    copy,
    screens::settings::Page,
    testing::{
        demo::demo_state,
        screens::{Fixture, Shown, WINDOW, harness_at},
    },
};

/// A window narrow enough for cards.
const CARDS: egui::Vec2 = egui::vec2(480.0, 620.0);

fn harness(size: egui::Vec2) -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Settings, demo_state());
    fixture.settings.page = Page::Appearance;
    let mut h = harness_at(fixture, size);
    h.run();
    h
}

fn flatten<'a>(shape: &'a Shape, into: &mut Vec<&'a Shape>) {
    if let Shape::Vec(shapes) = shape {
        for inner in shapes {
            flatten(inner, into);
        }
    } else {
        into.push(shape);
    }
}

/// How many rectangles the last frame filled with the theme's stripe tone.
fn stripe_fills(h: &Harness<'_, Fixture>) -> usize {
    let stripe = h.state().theme.current(&h.ctx).colors.row_stripe;
    let mut all = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut all);
    }
    all.into_iter()
        .filter(|shape| matches!(shape, Shape::Rect(r) if r.fill == stripe))
        .count()
}

fn click_switch(h: &mut Harness<'_, Fixture>) {
    h.get_by_role_and_label(Role::CheckBox, copy::STRIPED_ROWS)
        .click();
    h.run();
}

fn flips_the_stripes(size: egui::Vec2) {
    let mut h = harness(size);
    assert!(stripe_fills(&h) > 0, "striped by default");

    click_switch(&mut h);
    assert_eq!(stripe_fills(&h), 0, "switched off");

    click_switch(&mut h);
    assert!(stripe_fills(&h) > 0, "switched back on");
}

#[test]
fn the_switch_tints_and_clears_the_table_rows() {
    flips_the_stripes(WINDOW);
}

#[test]
fn the_switch_tints_and_clears_the_cards() {
    flips_the_stripes(CARDS);
}

#[test]
fn a_saved_off_choice_shows_no_stripes_until_the_switch_is_turned_on() {
    let mut state = demo_state();
    let mut config = (*state.config).clone();
    config.ui.striped_rows = false;
    state.config = std::sync::Arc::new(config);
    let mut fixture = Fixture::new(Shown::Settings, state);
    fixture.settings.page = Page::Appearance;
    let mut h = harness_at(fixture, WINDOW);
    h.run();
    assert_eq!(stripe_fills(&h), 0);
    click_switch(&mut h);
    assert!(stripe_fills(&h) > 0);
}
