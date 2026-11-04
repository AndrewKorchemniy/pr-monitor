//! Dialogs with a text input open with it focused, so the user can type at once. Focus is asked
//! for once, when the field appears: a click elsewhere keeps it, and Esc still closes the dialog
//! in one press.

use eframe::egui::{self, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};

use super::dialogs::{self, ROOMY, SMALLEST, dialog_harness, settings_fixture};
use crate::{
    copy,
    screens::{first_run::Step, settings::Editor},
    testing::{
        demo::demo_state,
        screens::{Fixture, Shown},
    },
};

/// The window sizes (points) and display scales the dialogs open at.
const WINDOWS: [(egui::Vec2, f32); 3] = [(ROOMY, 1.0), (ROOMY, 1.5), (SMALLEST, 1.0)];

fn open(fixture: impl Fn() -> Fixture) -> impl Iterator<Item = Harness<'static, Fixture>> {
    WINDOWS
        .into_iter()
        .map(move |(size, scale)| dialog_harness(fixture(), size, scale))
}

/// Whether any text input (a password field included) has the keyboard focus.
fn text_focused(h: &Harness<'_, Fixture>) -> bool {
    [Role::TextInput, Role::PasswordInput, Role::SearchInput]
        .into_iter()
        .any(|role| h.query_all_by_role(role).any(|node| node.is_focused()))
}

/// Whether the text input showing `placeholder` has the keyboard focus.
fn focused_with(h: &Harness<'_, Fixture>, placeholder: &str) -> bool {
    h.query_by(|n| n.data().placeholder() == Some(placeholder))
        .is_some_and(|node| node.is_focused())
}

/// Types `text` as the keyboard would, to whatever has the focus.
fn type_here(h: &mut Harness<'_, Fixture>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    h.run();
}

fn escape(h: &mut Harness<'_, Fixture>) {
    h.key_press(egui::Key::Escape);
    h.run();
}

fn shown(screen: Shown) -> Fixture {
    Fixture::new(screen, demo_state())
}

#[test]
fn every_dialog_with_a_text_input_opens_with_it_focused() {
    for h in open(|| shown(Shown::Token)) {
        assert!(text_focused(&h), "the token field");
    }
    for h in open(|| settings_fixture(Editor::AddOrg)) {
        assert!(focused_with(&h, copy::ORG_URL_HINT), "the add-org field");
    }
    for h in open(|| settings_fixture(Editor::Repositories)) {
        let hint = copy::search_scope(3, 10);
        assert!(focused_with(&h, &hint), "the scope search");
    }
    for h in open(|| settings_fixture(Editor::Reviewers)) {
        assert!(focused_with(&h, copy::ADD_PEOPLE), "the people field");
    }
    for h in open(|| shown(Shown::Palette)) {
        assert!(focused_with(&h, copy::PALETTE_HINT), "the palette");
    }
    for h in open(|| shown(Shown::FirstRun)) {
        assert!(focused_with(&h, copy::ORG_URL_HINT), "the first-run field");
    }
}

/// The first-run People step shows two people fields; the first one gets the focus.
#[test]
fn the_first_people_field_of_the_first_run_gets_the_focus() {
    let mut fixture = shown(Shown::FirstRun);
    fixture.first_run.step = Step::People;
    fixture.first_run.settings.draft.orgs = vec![fixture.state.config.orgs[0].clone()];
    let h = dialog_harness(fixture, ROOMY, 1.0);
    let fields: Vec<_> = h
        .query_all_by_role(Role::TextInput)
        .filter(|n| n.accesskit_node().placeholder() == Some(copy::ADD_PEOPLE))
        .collect();
    assert_eq!(fields.len(), 2);
    assert!(fields[0].is_focused() && !fields[1].is_focused());
}

/// With the field focused the app is in its text-input scope, so a typed letter is text, not a
/// shortcut (`dispatch_keys` reads exactly this).
#[test]
fn a_typed_letter_lands_in_the_field_not_in_a_shortcut() {
    let mut h = dialog_harness(settings_fixture(Editor::AddOrg), ROOMY, 1.0);
    assert!(h.ctx.egui_wants_keyboard_input());
    type_here(&mut h, "j");
    let field = dialogs::field(&h, copy::ORG_URL_HINT);
    assert_eq!(field.value().as_deref(), Some("j"));
    assert!(h.state_mut().take_actions().is_empty());

    let mut h = dialog_harness(shown(Shown::Token), ROOMY, 1.0);
    type_here(&mut h, "j");
    assert_eq!(h.state().token.0, "j");
}

/// Esc closes the dialog in one press, whether or not something was typed.
#[test]
fn esc_closes_a_dialog_in_one_press() {
    for text in ["", "abc"] {
        let mut h = dialog_harness(shown(Shown::Token), ROOMY, 1.0);
        type_here(&mut h, text);
        escape(&mut h);
        assert!(h.state().token_dismissed, "token, typed {text:?}");

        let mut h = dialog_harness(settings_fixture(Editor::AddOrg), ROOMY, 1.0);
        type_here(&mut h, text);
        escape(&mut h);
        let editing = h.state().settings.editing;
        assert_ne!(editing, Some(Editor::AddOrg), "add org, typed {text:?}");

        let mut h = dialog_harness(settings_fixture(Editor::Repositories), ROOMY, 1.0);
        type_here(&mut h, text);
        escape(&mut h);
        assert_eq!(h.state().settings.editing, None, "repositories, {text:?}");
    }

    // why: with a name typed the dropdown opens, and Esc closes only that (see people.rs).
    let mut h = dialog_harness(settings_fixture(Editor::Reviewers), ROOMY, 1.0);
    escape(&mut h);
    assert_eq!(h.state().settings.editing, None, "reviewers, empty field");
}

/// The focus is asked for when the field appears, not every frame: a click elsewhere keeps it.
#[test]
fn clicking_elsewhere_does_not_hand_the_focus_back() {
    let mut h = dialog_harness(shown(Shown::Token), ROOMY, 1.0);
    let title = h.query_all_by_label(copy::PAT_TITLE).last();
    title.expect("the title").click();
    for _ in 0..3 {
        h.step();
    }
    assert!(!text_focused(&h), "token");

    let mut h = dialog_harness(shown(Shown::Palette), ROOMY, 1.0);
    h.get_by_label(copy::PALETTE_FOOTER).click();
    for _ in 0..3 {
        h.step();
    }
    assert!(!focused_with(&h, copy::PALETTE_HINT), "palette");
}

/// The scope search is only there once the catalog is loaded and not everything is watched; it
/// takes the focus when it shows up, each time.
#[test]
fn the_scope_search_takes_the_focus_when_it_appears() {
    let hint = copy::search_scope(3, 10);
    let mut h = dialog_harness(settings_fixture(Editor::Repositories), ROOMY, 1.0);
    assert!(focused_with(&h, &hint));

    h.state_mut().settings.draft.orgs[0].scope.all_projects = true;
    h.run();
    assert!(
        h.query_by(|n| n.data().placeholder() == Some(hint.as_str()))
            .is_none()
    );

    h.state_mut().settings.draft.orgs[0].scope.all_projects = false;
    h.run();
    assert!(focused_with(&h, &hint), "the search came back");
}
