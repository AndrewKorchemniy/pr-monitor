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

use super::dialogs::{
    self, ROOMY, SMALLEST, assert_still, assert_title_still, dialog_harness, frames,
};
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
    let field = add_field(h);
    field.value().unwrap_or_default()
}

fn field_focused(h: &Harness<'_, Fixture>) -> bool {
    add_field(h).is_focused()
}

/// Types into the field and returns the search it sent.
fn type_query(h: &mut Harness<'_, Fixture>, text: &str) -> RequestId {
    add_field(h).type_text(text);
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
    add_field(&h).click();
    h.run();
    add_field(&h).type_text("pr");
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
    add_field(&h).click();
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
    add_field(&h).click();
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
    add_field(&h).click();
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
    add_field(&h).click();
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
    add_field(&h).click();
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
        let mut h = dialog_harness(dialogs::settings_fixture(editor), size, scale);
        add_field(&h).type_text("v");
        h.run();
        let mut places = Vec::new();
        for _ in 0..12 {
            h.step();
            let done = h.get_by_role_and_label(Role::Button, copy::DONE).rect();
            let field = add_field(&h).rect();
            places.push((done.min, field.min));
        }
        assert!(
            places.windows(2).all(|p| p[0] == p[1]),
            "{editor:?} at {size:?} x{scale}: {places:?}"
        );
    }
}

/// A person the directory knows, `n` apart from the others.
fn candidate(n: u128) -> PersonCandidate {
    Person {
        unique_name: Some(Arc::from(format!("person{n}@contoso.com"))),
        ..person(n, &format!("Person Number{n}"))
    }
    .to_candidate(CandidateSource::Harvested)
}

fn people_title(editor: Editor) -> (&'static str, &'static str) {
    (dialogs::editor_title(editor), copy::DONE)
}

fn add_field<'h>(h: &'h Harness<'_, Fixture>) -> egui_kittest::Node<'h> {
    dialogs::field(h, copy::ADD_PEOPLE)
}

/// Searches for "per", has the directory answer with person `n`, and picks them with Enter.
fn add_person(h: &mut Harness<'_, Fixture>, n: u128) {
    let request = type_query(h, "per");
    deliver(h, request, &[candidate(n)]);
    h.key_press(egui::Key::Enter);
    h.run();
}

/// Typing opened the dropdown (and its spacer), so the dialog grew a frame later and its top
/// moved on the next blink of the cursor.
#[test]
fn the_title_holds_still_while_typing() {
    for editor in [Editor::Tracked, Editor::Reviewers, Editor::Ignored] {
        for (size, scale) in dialogs::windows() {
            let mut h = dialog_harness(dialogs::settings_fixture(editor), size, scale);
            let mut seen = frames(&mut h, 2, people_title(editor));
            add_field(&h).click();
            for letter in ["p", "r", "i", "y"] {
                add_field(&h).type_text(letter);
                seen.extend(frames(&mut h, 4, people_title(editor)));
            }
            assert_title_still(&seen, &format!("{editor:?} at {size:?} x{scale}"));
        }
    }
}

/// The matches changing under the field (the hint, "Searching...", rows, "No people match")
/// must not resize the dialog.
#[test]
fn the_dialog_keeps_its_size_as_results_arrive_and_change() {
    for size in [ROOMY, SMALLEST] {
        let title = people_title(Editor::Reviewers);
        let mut h = dialog_harness(dialogs::settings_fixture(Editor::Reviewers), size, 1.25);
        let mut seen = frames(&mut h, 2, title);
        add_field(&h).click();
        let request = type_query(&mut h, "pri");
        seen.extend(frames(&mut h, 3, title));
        let batches: [Vec<PersonCandidate>; 3] = [
            vec![priya(), priyanka()],
            (20..28).map(candidate).collect(),
            Vec::new(),
        ];
        for found in &batches {
            deliver(&mut h, request, found);
            seen.extend(frames(&mut h, 3, title));
        }
        h.key_press(egui::Key::Escape);
        seen.extend(frames(&mut h, 3, title));
        assert_still(&seen, &format!("{size:?}"));
    }
}

/// Each added person is another pill and, until the list scrolls, another line.
#[test]
fn adding_ten_people_does_not_move_or_resize_the_dialog() {
    for size in [ROOMY, SMALLEST] {
        let title = people_title(Editor::Reviewers);
        let mut h = dialog_harness(dialogs::settings_fixture(Editor::Reviewers), size, 1.0);
        let mut seen = frames(&mut h, 2, title);
        add_field(&h).click();
        for n in 0..10 {
            let request = type_query(&mut h, "per");
            seen.extend(frames(&mut h, 2, title));
            deliver(&mut h, request, &[candidate(40 + n)]);
            h.key_press(egui::Key::Enter);
            seen.extend(frames(&mut h, 3, title));
        }
        assert_eq!(reviewers(&h).len(), 12);
        assert_still(&seen, &format!("{size:?}"));
        // why: the newest person is scrolled into view, above the dialog's buttons.
        let newest = copy::remove_person("Person Number49");
        let pill = h
            .query_all_by_label(&newest)
            .last()
            .expect("the newest pill");
        let done = h.get_by_role_and_label(Role::Button, copy::DONE).rect();
        assert!(
            pill.rect().max.y <= done.min.y,
            "{size:?}: {:?}",
            pill.rect()
        );
    }
}

/// Every editor had the area id `edit_people`, so one opened at the size of the last.
#[test]
fn each_editor_opens_at_its_own_place() {
    let title = people_title(Editor::Tracked);
    let mut fresh = dialog_harness(dialogs::settings_fixture(Editor::Tracked), ROOMY, 1.0);
    let fresh = frames(&mut fresh, 3, title);
    let mut h = dialog_harness(dialogs::settings_fixture(Editor::Reviewers), ROOMY, 1.0);
    add_field(&h).click();
    for n in 0..10 {
        add_person(&mut h, 60 + n);
    }
    h.state_mut().settings.editing = None;
    h.run();
    h.state_mut().settings.editing = Some(Editor::Tracked);
    let opened = frames(&mut h, 3, title);
    assert_eq!(opened, fresh, "the first frames of a second editor");
}
