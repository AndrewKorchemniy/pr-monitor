//! The card list in a narrow window: cards answer the pointer and the keys like table rows, group
//! headers collapse, the toolbar sorts, and switching layouts keeps the user's place.

use eframe::egui::{
    self,
    accesskit::{Role, Toggled},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::view::{GroupBy, SortKey};

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    testing::{
        demo::{demo_state, now},
        reviewed, sample_rows,
        screens::{Fixture, NARROW, Shown, WINDOW, harness_at},
        state_with,
    },
    view::DashboardView,
};

/// A window narrow enough for cards.
const CARDS: egui::Vec2 = egui::vec2(480.0, 620.0);

fn cards(fixture: Fixture) -> Harness<'static, Fixture> {
    let mut h = harness_at(fixture, CARDS);
    h.run();
    assert!(
        h.query_by_role_and_label(Role::Button, copy::COLUMN_TITLE)
            .is_none(),
        "the table's header is not shown"
    );
    h
}

fn sample() -> Fixture {
    Fixture::new(Shown::Dashboard, state_with(sample_rows()))
}

fn has(actions: &[UiAction], wanted: impl Fn(&UiAction) -> bool) -> bool {
    actions.iter().any(wanted)
}

#[test]
fn clicking_a_card_selects_it() {
    let mut h = cards(sample());
    h.get_by_label("Add retry").click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        has(
            &actions,
            |a| matches!(a, UiAction::Select(k) if *k == reviewed())
        ),
        "{actions:?}"
    );
}

#[test]
fn double_clicking_a_card_opens_it() {
    let mut h = cards(sample());
    let at = h.get_by_label("Add retry").rect().center();
    h.event(egui::Event::PointerMoved(at));
    h.step();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        has(
            &actions,
            |a| matches!(a, UiAction::Open(k) if *k == reviewed())
        ),
        "{actions:?}"
    );
}

#[test]
fn right_clicking_a_card_offers_the_rows_menu() {
    let mut h = cards(sample());
    h.get_by_label("Add retry").click_secondary();
    h.run();
    for item in [copy::COPY_LINK, copy::COPY_TITLE, copy::PEEK] {
        h.get_by_label(item);
    }
    h.get_by_label(copy::OPEN_IN_BROWSER).click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        has(
            &actions,
            |a| matches!(a, UiAction::Select(k) if *k == reviewed())
        ),
        "{actions:?}"
    );
    assert!(
        has(&actions, |a| matches!(a, UiAction::Run(CommandId::PrOpen))),
        "{actions:?}"
    );
}

#[test]
fn a_card_shows_its_facts_review_marks_and_flags() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.now = now();
    // why: tall enough that every card is built.
    let mut h = harness_at(fixture, egui::vec2(CARDS.x, 1400.0));
    h.run();
    for fact in ["!4699", copy::DRAFT, copy::AUTO_COMPLETE, copy::CONFLICTS] {
        h.get_by_label(fact);
    }
    h.get_by_label("Rita Okafor: Approved · Sam Whitfield: Approved with suggestions");
    assert_eq!(h.query_all_by_label("Leo Martins").count(), 2);
    assert_eq!(h.query_all_by_label("telemetry").count(), 3);
}

#[test]
fn card_group_headers_collapse_like_the_tables() {
    let mut h = cards(sample());
    h.get_by_role_and_label(Role::Button, "Active comments")
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        has(&actions, |a| matches!(a, UiAction::ToggleGroup(1))),
        "{actions:?}"
    );
}

#[test]
fn the_sort_menu_sorts_by_a_column_and_says_which_way() {
    let mut h = cards(sample());
    let name = copy::menu_value(copy::SORT, copy::COLUMN_STATUS);
    h.get_by_role_and_label(Role::ComboBox, &name).click();
    h.run();
    for column in [copy::COLUMN_NUMBER, copy::COLUMN_TITLE, copy::COLUMN_REPO] {
        let item = h.get_by_role_and_label(Role::MenuItem, column);
        assert_eq!(
            item.accesskit_node().toggled(),
            Some(Toggled::False),
            "{column}"
        );
    }
    let current = h.get_by_role_and_label(Role::MenuItem, copy::COLUMN_STATUS);
    assert_eq!(current.accesskit_node().toggled(), Some(Toggled::True));
    h.get_by_role_and_label(Role::MenuItem, copy::COLUMN_AGE)
        .click();
    h.run();
    let actions = h.state_mut().take_actions();
    assert!(
        has(&actions, |a| matches!(a, UiAction::Sort(SortKey::Age))),
        "{actions:?}"
    );
}

/// The list's top and bottom: under the toolbar, above the status bar.
fn list_span(h: &Harness<'_, Fixture>) -> (f32, f32) {
    let theme = crate::theme::fallback();
    let toolbar = h.get_by_label(CommandId::SettingsOpen.title()).rect();
    let summary = h.state().state.summary;
    let count = copy::status_count(summary.needs_review, prm_domain::PrStatus::NeedsReview);
    let status = h.get_by_label(&count).rect();
    let top = toolbar.center().y + theme.toolbar_height() / 2.0;
    let bottom = status.center().y - theme.tokens.metrics.row.statusbar_height / 2.0;
    (top, bottom)
}

/// The card or row titled `title` lies wholly inside the list.
fn assert_in_view(h: &Harness<'_, Fixture>, title: &str, case: &str) {
    let (top, bottom) = list_span(h);
    let rect = h.get_by_label(title).rect();
    assert!(
        rect.top() >= top && rect.bottom() <= bottom,
        "{case}: {title} at {rect:?}, the list is {top}..{bottom}"
    );
}

/// What a key does to the view (the app maps the keys to these).
type Press = fn(&mut DashboardView);

fn selected_title(h: &Harness<'_, Fixture>) -> String {
    let row = h.state().dash.view.selected_row();
    row.map(|r| r.title.to_string()).unwrap_or_default()
}

#[test]
fn keys_move_the_selection_and_the_selected_card_scrolls_into_view() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    fixture.dash.view.set_group(GroupBy::None);
    let mut h = harness_at(fixture, NARROW);
    h.run();
    let keys: [(&str, Press); 4] = [
        ("End", |v| v.nav_edge(true)),
        ("Home", |v| v.nav_edge(false)),
        ("Page down", |v| v.nav_page(true)),
        ("Up", |v| v.nav(-1)),
    ];
    for (key, press) in keys {
        press(&mut h.state_mut().dash.view);
        h.run();
        assert_in_view(&h, &selected_title(&h), key);
    }
}

#[test]
fn switching_between_table_and_cards_keeps_the_selection_in_view() {
    let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
    let title = "Bump windows-rs to 0.62";
    let key = fixture
        .state
        .rows
        .iter()
        .find(|r| r.title.as_ref() == title)
        .map(|r| r.key.clone());
    if let Some(key) = key {
        fixture.dash.view.select_key(&key);
    }
    let height = 420.0;
    let mut h = harness_at(fixture, egui::vec2(WINDOW.x, height));
    h.run();
    assert_in_view(&h, title, "table");
    for width in [CARDS.x, WINDOW.x, NARROW.x] {
        h.set_size(egui::vec2(width, height));
        h.run();
        assert_in_view(&h, title, &format!("at {width} pt"));
    }
}

#[test]
fn a_long_title_wraps_to_two_lines_on_its_card() {
    let long = "Credential Manager chunking for long tokens, so sign-ins that outgrow one entry \
                survive a restart";
    for width in [NARROW.x, CARDS.x] {
        let mut fixture = Fixture::new(Shown::Dashboard, demo_state());
        fixture.dash.view.set_group(GroupBy::None);
        let mut h = harness_at(fixture, egui::vec2(width, 1400.0));
        h.run();
        let line = h.get_by_label("Low-power hover bands").rect().height();
        let title = h.get_by_label(long).rect();
        let lines = title.height() / line;
        assert!(
            (1.5..=2.05).contains(&lines),
            "at {width} pt the title takes {lines} lines: {title:?}"
        );
        assert!(title.right() <= width, "at {width} pt: {title:?}");
    }
}
