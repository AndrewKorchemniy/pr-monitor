use std::sync::Arc;

use prm_domain::{
    OrgId, Person,
    ids::RequestId,
    person::{CandidateSource, PersonCandidate},
    testing::{org, person},
};
use prm_service::{AppEvent, SearchPhase, UiError};

use super::*;

fn known(n: u128, name: &str, unique: &str) -> PersonCandidate {
    Person {
        unique_name: Some(Arc::from(unique)),
        ..person(n, name)
    }
    .to_candidate(CandidateSource::Harvested)
}

fn directory(descriptor: &str, name: &str, unique: &str) -> PersonCandidate {
    PersonCandidate {
        id: None,
        descriptor: Some(Arc::from(descriptor)),
        name: Arc::from(name),
        unique_name: Some(Arc::from(unique)),
        group: false,
        source: CandidateSource::Directory,
    }
}

fn contoso() -> OrgId {
    org("contoso")
}

fn resolved(request: RequestId, n: u128, name: &str) -> AppEvent {
    AppEvent::PersonResolved {
        request,
        org: contoso(),
        result: Ok(person(n, name)),
    }
}

fn results(request: RequestId, phase: SearchPhase, found: &[PersonCandidate]) -> AppEvent {
    AppEvent::PeopleResults {
        request,
        org: contoso(),
        query: Arc::from("maya"),
        phase,
        candidates: Arc::from(found),
    }
}

/// A picker that typed "maya" and is waiting for its search.
fn searching() -> (PeoplePicker, RequestId) {
    let mut picker = PeoplePicker::new("tracked");
    picker.query = "maya".to_owned();
    let request = picker.typed().expect("4 letters start a search");
    (picker, request)
}

#[test]
fn stepped_wraps_at_both_ends_and_clamps_a_stale_highlight() {
    let cases = [
        (0, Step::Down, 3, 1),
        (2, Step::Down, 3, 0),
        (0, Step::Up, 3, 2),
        (1, Step::Up, 3, 0),
        (7, Step::Up, 3, 1),
        (7, Step::Down, 3, 0),
        (0, Step::Down, 1, 0),
        (4, Step::Down, 0, 0),
        (4, Step::Up, 0, 0),
    ];
    for (highlight, step, count, expected) in cases {
        assert_eq!(
            stepped(highlight, step, count),
            expected,
            "{step:?} from {highlight} among {count}"
        );
    }
}

#[test]
fn offered_hides_people_already_chosen_and_caps_the_rows() {
    let maya = Person {
        unique_name: Some(Arc::from("maya@contoso.com")),
        ..person(1, "Maya Patel")
    };
    let mut found = vec![
        known(1, "Maya Patel", "maya@contoso.com"),
        directory("aad.maya", "Maya P.", "MAYA@contoso.com"),
        known(2, "Jonas Berg", "jonas@contoso.com"),
    ];
    found.extend((10..20).map(|n| known(n, "Priya Raman", &format!("priya{n}@contoso.com"))));

    let names: Vec<&str> = offered(&found, std::slice::from_ref(&maya))
        .map(|c| c.name.as_ref())
        .collect();

    assert_eq!(names.len(), MAX_ROWS);
    assert_eq!(names[0], "Jonas Berg");
    assert!(!names.contains(&"Maya Patel") && !names.contains(&"Maya P."));
}

#[test]
fn typed_shows_the_hint_below_three_letters_then_searches() {
    let mut picker = PeoplePicker::new("tracked");
    picker.query = "ma".to_owned();
    assert_eq!(picker.typed(), None);
    assert!(picker.open);
    assert_eq!(picker.search, Search::Idle);

    picker.query = "  ".to_owned();
    assert_eq!(picker.typed(), None);
    assert!(!picker.open, "an empty field closes the dropdown");

    picker.query = "may".to_owned();
    let request = picker.typed();
    assert!(request.is_some());
    assert_eq!(picker.search, Search::Searching(request.unwrap()));
}

#[test]
fn results_fill_in_local_first_then_finish_and_stale_ones_are_ignored() {
    let (mut picker, request) = searching();
    let local = [known(1, "Maya Patel", "maya@contoso.com")];
    let merged = [
        local[0].clone(),
        directory("aad.mp", "Maya Park", "mpark@contoso.com"),
    ];

    picker.on_event(&results(RequestId(u64::MAX), SearchPhase::Final, &merged));
    assert_eq!(picker.results.len(), 0, "a stale request is ignored");
    picker.on_event(&results(request, SearchPhase::Local, &local));
    assert_eq!(picker.results.len(), 1);
    assert_eq!(picker.search, Search::Searching(request));
    picker.on_event(&results(request, SearchPhase::Final, &merged));

    assert_eq!(picker.results.len(), 2);
    assert!(matches!(picker.search, Search::Done(_)));
}

#[test]
fn failed_search_keeps_the_local_matches_and_says_why() {
    let (mut picker, request) = searching();
    let local = [known(1, "Maya Patel", "maya@contoso.com")];
    let failed = SearchPhase::Failed(UiError::new("Couldn't search the directory"));

    picker.on_event(&results(request, failed, &local));

    assert_eq!(picker.results.len(), 1);
    assert_eq!(
        picker.search,
        Search::Failed(Arc::from("Couldn't search the directory"))
    );
}

#[test]
fn highlighted_follows_the_steps_over_the_offered_matches() {
    let (mut picker, request) = searching();
    let found = [
        known(1, "Maya Patel", "maya@contoso.com"),
        known(2, "Maya Lind", "mlind@contoso.com"),
        known(3, "Mayank Rao", "mrao@contoso.com"),
    ];
    picker.on_event(&results(request, SearchPhase::Final, &found));
    let chosen = [person(1, "Maya Patel")];
    let count = offered(&picker.results, &chosen).count();

    picker.step(Step::Down, count);
    let after_down = picker.highlighted(&chosen).map(|c| c.name.clone());
    picker.step(Step::Down, count);
    let wrapped = picker.highlighted(&chosen).map(|c| c.name.clone());

    assert_eq!(after_down.as_deref(), Some("Mayank Rao"));
    assert_eq!(wrapped.as_deref(), Some("Maya Lind"));
}

#[test]
fn picking_a_known_person_adds_them_and_clears_the_field() {
    let (mut picker, _) = searching();
    let maya = known(1, "Maya Patel", "maya@contoso.com");

    let pick = picker.pick(&maya);

    assert_eq!(pick, Pick::Add(maya.to_person().unwrap()));
    assert_eq!(picker.query, "");
    assert!(!picker.open, "the dropdown closes for the next name");
    assert_eq!(picker.search, Search::Idle);
}

#[test]
fn picking_a_directory_entry_shows_adding_until_it_resolves() {
    let (mut picker, _) = searching();
    let entry = directory("aad.mp", "Maya Park", "mpark@contoso.com");

    let Pick::Resolve(request, candidate) = picker.pick(&entry) else {
        panic!("a directory entry needs resolving");
    };
    assert_eq!(candidate, entry);
    assert!(picker.open && picker.resolving == [request]);
    let resolved = Person {
        unique_name: Some(Arc::from("mpark@contoso.com")),
        ..person(9, "Maya Park")
    };
    let event = AppEvent::PersonResolved {
        request,
        org: contoso(),
        result: Ok(resolved.clone()),
    };

    assert_eq!(picker.on_event(&event), Some(resolved));
    assert_eq!(picker.resolving, Vec::<RequestId>::new());
    assert!(!picker.open);
}

#[test]
fn a_failed_resolve_stays_open_with_the_reason() {
    let (mut picker, _) = searching();
    let Pick::Resolve(request, _) = picker.pick(&directory("aad.mp", "Maya Park", "m@x")) else {
        panic!("a directory entry needs resolving");
    };
    picker.close();
    let event = AppEvent::PersonResolved {
        request,
        org: contoso(),
        result: Err(UiError::new("Couldn't look up this person")),
    };

    assert_eq!(picker.on_event(&event), None);
    assert!(picker.open);
    assert_eq!(
        picker.error.as_deref(),
        Some("Couldn't look up this person")
    );
}

#[test]
fn picks_made_back_to_back_are_all_added_whatever_order_they_resolve_in() {
    let (mut picker, _) = searching();
    let Pick::Resolve(first, _) = picker.pick(&directory("aad.mp", "Maya Park", "mp@x")) else {
        panic!("a directory entry needs resolving");
    };
    picker.query = "jon".to_owned();
    picker.typed();
    let Pick::Resolve(second, _) = picker.pick(&directory("aad.jb", "Jonas Berg", "jb@x")) else {
        panic!("a directory entry needs resolving");
    };
    assert!(picker.open, "the dropdown says Adding… while both resolve");

    let jonas = picker.on_event(&resolved(second, 2, "Jonas Berg"));
    assert!(picker.open, "Maya is still being added");
    let maya = picker.on_event(&resolved(first, 1, "Maya Park"));

    assert_eq!(jonas.map(|p| p.name), Some(Arc::from("Jonas Berg")));
    assert_eq!(maya.map(|p| p.name), Some(Arc::from("Maya Park")));
    assert_eq!(picker.resolving, Vec::<RequestId>::new());
    assert!(!picker.open);
    assert_eq!(picker.on_event(&resolved(first, 1, "Maya Park")), None);
}

#[test]
fn reset_forgets_the_search_but_still_adds_a_pending_pick() {
    let (mut picker, request) = searching();
    picker.on_event(&results(
        request,
        SearchPhase::Final,
        &[directory("aad.mp", "Maya Park", "mp@x")],
    ));
    let Pick::Resolve(pending, _) = picker.pick(&directory("aad.mp", "Maya Park", "mp@x")) else {
        panic!("a directory entry needs resolving");
    };
    picker.query = "pri".to_owned();
    picker.typed();
    picker.error = Some(Arc::from("Couldn't look up this person"));

    picker.reset();

    assert_eq!(picker.query, "");
    assert_eq!(picker.search, Search::Idle);
    assert!(picker.results.is_empty() && picker.error.is_none() && !picker.open);
    let added = picker.on_event(&resolved(pending, 9, "Maya Park"));
    assert_eq!(added.map(|p| p.name), Some(Arc::from("Maya Park")));
}

#[test]
fn typing_on_keeps_the_matches_that_still_fit_until_new_results_arrive() {
    let (mut picker, request) = searching();
    let park = directory("aad.mp", "Maya Park", "mpark@contoso.com");
    let found = [known(1, "Maya Patel", "maya@contoso.com"), park.clone()];
    picker.on_event(&results(request, SearchPhase::Final, &found));

    let names =
        |p: &PeoplePicker| -> Vec<Arc<str>> { p.results.iter().map(|c| c.name.clone()).collect() };
    picker.query = "maya pa".to_owned();
    picker.typed();
    assert_eq!(names(&picker).len(), 2, "both still fit");
    picker.query = "maya par".to_owned();
    let next = picker.typed().expect("still a search");
    assert_eq!(names(&picker), [Arc::from("Maya Park")]);

    let local = [known(3, "Maya Parr", "mparr@contoso.com")];
    picker.on_event(&results(next, SearchPhase::Local, &local));
    assert_eq!(
        names(&picker),
        [Arc::from("Maya Parr"), Arc::from("Maya Park")],
        "local first, then the earlier directory match"
    );
    picker.on_event(&results(next, SearchPhase::Final, &local));
    assert_eq!(
        names(&picker),
        [Arc::from("Maya Parr")],
        "the directory has the last word"
    );

    picker.query = "ma".to_owned();
    picker.typed();
    assert!(
        picker.results.is_empty(),
        "under 3 letters there is nothing to keep"
    );
}

#[test]
fn a_signed_out_search_says_the_directory_was_not_searched() {
    let (mut picker, request) = searching();

    picker.on_event(&results(request, SearchPhase::LocalOnly, &[]));

    assert_eq!(
        picker.search,
        Search::LocalOnly(Arc::from(copy::SIGN_IN_TO_SEARCH))
    );
}
