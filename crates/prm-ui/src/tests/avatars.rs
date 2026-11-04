//! Faces in the dashboard: the author in the table and the reviewers in the peek. Pictures or
//! initials, each is an image named after the person.

use eframe::egui::{self, Role};
use egui_kittest::kittest::Queryable as _;

use crate::testing::{
    demo::{demo_state, now},
    screens::{Fixture, Shown, WINDOW, harness_at},
};

fn images(h: &egui_kittest::Harness<'_, Fixture>, name: &str) -> usize {
    h.query_all_by_role_and_label(Role::Image, name).count()
}

fn dashboard(size: egui::Vec2, peek: bool) -> egui_kittest::Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    fixture.dash.peek.open = peek;
    let with_reviewers = fixture.state.rows.iter().find(|r| !r.reviewers.is_empty());
    if let Some(key) = with_reviewers.map(|r| r.key.clone()) {
        fixture.dash.view.select_key(&key);
    }
    let mut h = harness_at(fixture, size);
    h.run();
    h
}

#[test]
fn the_table_shows_each_authors_face() {
    let h = dashboard(WINDOW, false);
    // A person with a picture and one with initials alike.
    assert!(images(&h, "Maya Patel") >= 1, "has a picture");
    assert!(images(&h, "Priya Raman") >= 1, "shows initials");
}

#[test]
fn the_peek_lists_reviewers_with_faces() {
    let h = dashboard(WINDOW, true);
    let rows = h.state().state.rows.clone();
    let reviewers: Vec<_> = rows
        .iter()
        .find(|r| Some(&r.key) == h.state().dash.view.selected.as_ref())
        .map(|r| r.reviewers.iter().map(|c| c.name.to_string()).collect())
        .unwrap_or_default();
    assert!(!reviewers.is_empty(), "a demo PR has reviewers");
    for name in reviewers {
        assert!(images(&h, &name) >= 1, "{name}");
    }
}

#[test]
fn a_loaded_picture_is_what_the_face_shows() {
    let h = dashboard(WINDOW, false);
    let state = h.state();
    for picture in crate::testing::demo::demo_avatars() {
        assert!(matches!(
            crate::widgets::avatar::face(&state.avatars, &picture.key),
            crate::widgets::avatar::Face::Image(_)
        ));
    }
}
