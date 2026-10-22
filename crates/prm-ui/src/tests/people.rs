//! The people combobox in the settings dialog: typing searches, the keys move and pick, a pick
//! keeps the field focused for the next name, and Esc closes only the dropdown.

use std::sync::Arc;

use eframe::egui::{self, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::{
    Person,
    ids::RequestId,
    person::{CandidateSource, PersonCandidate},
    testing::{org, person},
};
use prm_service::{AppEvent, Command, SearchPhase};

use crate::{
    actions::UiAction,
    copy,
    screens::settings::{Editor, Page},
    testing::{
        demo::demo_state,
        screens::{Fixture, Shown, harness},
    },
};

fn priya() -> PersonCandidate {
    Person {
        unique_name: Some(Arc::from("priya@contoso.com")),
        ..person(3, "Priya Raman")
    }
    .to_candidate(CandidateSource::Harvested)
}

fn priyanka() -> PersonCandidate {
    PersonCandidate {
        id: None,
        descriptor: Some(Arc::from("aad.priyanka")),
        name: Arc::from("Priyanka Das"),
        unique_name: Some(Arc::from("priyanka@contoso.com")),
        group: false,
        source: CandidateSource::Directory,
    }
}

/// The reviewers dialog of the demo org (Rita and Sam already count).
fn reviewers_dialog() -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Settings, demo_state());
    fixture.settings.page = Page::Organizations;
    fixture.settings.editing = Some(Editor::Reviewers);
    let mut h = harness(fixture);
    h.run();
    h
}

fn field_text(h: &Harness<'_, Fixture>) -> String {
    let field = h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE));
    field.value().unwrap_or_default()
}

fn field_focused(h: &Harness<'_, Fixture>) -> bool {
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .is_focused()
}

/// Types into the field and returns the search it sent.
fn type_query(h: &mut Harness<'_, Fixture>, text: &str) -> RequestId {
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .type_text(text);
    h.run();
    let actions = h.state_mut().take_actions();
    actions
        .iter()
        .rev()
        .find_map(|action| {
            if let UiAction::Send(Command::SearchPeople { request, .. }) = action {
                Some(*request)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("no search sent: {actions:?}"))
}

fn deliver(h: &mut Harness<'_, Fixture>, request: RequestId, found: &[PersonCandidate]) {
    h.state_mut().settings.on_event(&AppEvent::PeopleResults {
        request,
        org: org("contoso"),
        query: Arc::from("pri"),
        phase: SearchPhase::Final,
        candidates: Arc::from(found),
    });
    h.run();
}

fn reviewers(h: &Harness<'_, Fixture>) -> Vec<String> {
    h.state().settings.draft.orgs[0]
        .people
        .reviewers
        .iter()
        .map(|p| p.name.to_string())
        .collect()
}

#[test]
fn typing_opens_the_dropdown_with_the_hint_then_the_matches() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .type_text("pr");
    h.run();
    h.get_by_label(copy::SEARCH_MIN);

    let request = type_query(&mut h, "i");
    h.get_by_label(copy::SEARCHING);
    deliver(&mut h, request, &[priya(), priyanka()]);

    h.get_by_role_and_label(Role::MenuItem, "Priya Raman");
    h.get_by_role_and_label(Role::MenuItem, "Priyanka Das");
    assert!(h.query_by_label(copy::SEARCHING).is_none());
}

#[test]
fn enter_adds_the_highlighted_person_and_keeps_typing_in_the_field() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    let request = type_query(&mut h, "pri");
    deliver(&mut h, request, &[priya(), priyanka()]);

    h.key_press(egui::Key::Enter);
    h.run();

    assert_eq!(
        reviewers(&h),
        ["Rita Okafor", "Sam Whitfield", "Priya Raman"]
    );
    assert_eq!(field_text(&h), "");
    assert!(field_focused(&h), "the field keeps focus for the next name");
    assert!(
        h.query_by_role_and_label(Role::MenuItem, "Priyanka Das")
            .is_none()
    );
}

#[test]
fn people_already_added_are_not_offered_and_arrows_pick_a_directory_entry() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    let rita = person(7, "Rita Okafor").to_candidate(CandidateSource::Configured);
    let request = type_query(&mut h, "pri");
    deliver(&mut h, request, &[rita, priya(), priyanka()]);
    assert!(
        h.query_by_role_and_label(Role::MenuItem, "Rita Okafor")
            .is_none()
    );

    h.key_press(egui::Key::ArrowDown);
    h.key_press(egui::Key::Enter);
    h.run();

    let actions = h.state_mut().take_actions();
    let resolving = actions.iter().any(|action| {
        matches!(action, UiAction::Send(Command::ResolvePerson { candidate, .. })
            if candidate.name.as_ref() == "Priyanka Das")
    });
    assert!(resolving, "{actions:?}");
    h.get_by_label(copy::ADDING);
    assert!(field_focused(&h));
    assert_eq!(reviewers(&h).len(), 2, "added once it resolves");
}

#[test]
fn esc_closes_the_dropdown_and_keeps_the_text() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    let request = type_query(&mut h, "pri");
    deliver(&mut h, request, &[priya()]);

    h.key_press(egui::Key::Escape);
    h.run();

    assert!(
        h.query_by_role_and_label(Role::MenuItem, "Priya Raman")
            .is_none()
    );
    assert_eq!(field_text(&h), "pri");
    assert!(field_focused(&h), "Esc closes the dropdown, not the field");
    assert_eq!(h.state().settings.editing, Some(Editor::Reviewers));

    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(
        h.state().settings.editing,
        None,
        "a second Esc closes the dialog"
    );
}

#[test]
fn clicking_a_match_adds_it_and_a_pill_removes_it() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    let request = type_query(&mut h, "pri");
    deliver(&mut h, request, &[priya()]);

    h.get_by_role_and_label(Role::MenuItem, "Priya Raman")
        .click();
    h.run();
    assert_eq!(
        reviewers(&h),
        ["Rita Okafor", "Sam Whitfield", "Priya Raman"]
    );
    assert!(field_focused(&h));

    // why: Rita's pill also shows on the page behind the dialog; the dialog's comes last.
    let name = copy::remove_person("Rita Okafor");
    let removes: Vec<_> = h.query_all_by_label(&name).collect();
    removes.last().expect("Rita's pill in the dialog").click();
    h.run();
    assert_eq!(reviewers(&h), ["Sam Whitfield", "Priya Raman"]);
}

#[test]
fn closing_the_dialog_clears_the_field_for_next_time() {
    let mut h = reviewers_dialog();
    h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
        .click();
    h.run();
    let request = type_query(&mut h, "pri");
    deliver(&mut h, request, &[priya()]);

    h.get_by_label(copy::DONE).click();
    h.run();
    assert_eq!(h.state().settings.editing, None);
    h.state_mut().settings.editing = Some(Editor::Reviewers);
    h.run();

    assert_eq!(field_text(&h), "");
    assert!(
        h.query_by_role_and_label(Role::MenuItem, "Priya Raman")
            .is_none()
    );
}

/// With the dropdown open, the dialog moved sideways and back on every frame (each blink of the
/// text cursor is a frame), so the layout never settled. Display scales make it fractional.
#[test]
fn the_dialog_holds_still_while_the_dropdown_is_open() {
    let cases = [
        (Editor::Reviewers, egui::vec2(1280.0, 800.0)),
        (Editor::Tracked, egui::vec2(1280.0, 800.0)),
        (Editor::Tracked, egui::vec2(1003.0, 731.0)),
        (Editor::Tracked, egui::vec2(1537.0, 863.0)),
    ];
    for ((editor, size), scale) in cases
        .into_iter()
        .flat_map(|c| [1.0, 1.25, 1.5].map(|s| (c, s)))
    {
        let mut fixture = Fixture::new(Shown::Settings, demo_state());
        fixture.settings.page = Page::Organizations;
        fixture.settings.editing = Some(editor);
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(scale)
            .with_step_dt(0.3)
            .with_context_setup(crate::theme::fonts::install)
            .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture);
        // why: egui paints (and blinks) the text cursor only in a focused window.
        h.event(egui::Event::WindowFocused(true));
        h.run();
        h.get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
            .type_text("v");
        h.run();
        let mut places = Vec::new();
        for _ in 0..12 {
            h.step();
            let done = h.get_by_role_and_label(Role::Button, copy::DONE).rect();
            let field = h
                .get_by(|n| n.data().placeholder() == Some(copy::ADD_PEOPLE))
                .rect();
            places.push((done.min, field.min));
        }
        assert!(
            places.windows(2).all(|p| p[0] == p[1]),
            "{editor:?} at {size:?} x{scale}: {places:?}"
        );
    }
}
