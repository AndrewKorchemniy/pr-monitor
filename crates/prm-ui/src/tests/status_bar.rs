//! The status bar in narrow windows: what gives way, what never does, and that nothing overlaps
//! or comes and goes while text ticks.

use std::{sync::Arc, time::Duration};

use eframe::egui::{
    self,
    accesskit::{Role, Toggled},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use prm_domain::{PrStatus, view::StatusFilter};
use prm_service::AppState;

use crate::{
    copy,
    format::{Countdown, countdown},
    testing::{
        demo::{busy_state, crowded_state, demo_state, now},
        screens::{Fixture, NARROW, Shown, WINDOW, harness_at},
    },
    view::DashboardView,
    widgets::status_bar::updated_text,
};

/// Sets up the dashboard's filters before it is drawn.
type Setup = fn(&mut DashboardView);

/// How far apart the swept window widths are, in points.
const SWEEP_STEP: f32 = 10.0;

/// Window widths from the smallest the app allows up to the default one.
fn widths() -> impl Iterator<Item = f32> {
    std::iter::successors(Some(NARROW.x), |w| Some(w + SWEEP_STEP)).take_while(|w| *w <= WINDOW.x)
}

/// The dashboard in a window `width` points wide, at the demo's "now".
fn dashboard(state: AppState, setup: Setup, width: f32) -> Harness<'static, Fixture> {
    let mut fixture = Fixture::new(Shown::Dashboard, state);
    fixture.now = now();
    setup(&mut fixture.dash.view);
    let mut h = harness_at(fixture, egui::vec2(width, NARROW.y));
    h.run();
    h
}

fn unfiltered(_: &mut DashboardView) {}

/// The bar's buttons and labels with their text and bounds, left to right.
fn segments(h: &Harness<'_, Fixture>) -> Vec<(String, egui::Rect)> {
    let summary = h.state().state.summary;
    let anchor = copy::status_count(summary.needs_review, PrStatus::NeedsReview);
    // why: the counts' row holds the whole bar, the right side nested in it. Table rows
    // clipped behind the bar share its height, so a query by position would find them too.
    let row = h.get_by_label(&anchor).parent().expect("the bar's row");
    let mut found: Vec<_> = row
        .children_recursive()
        .filter(|node| matches!(node.accesskit_node().role(), Role::Button | Role::Label))
        .map(|node| (text(&node), node.rect()))
        .collect();
    found.sort_by(|a, b| a.1.min.x.total_cmp(&b.1.min.x));
    found
}

/// A button's label, or a label's text.
fn text(node: &egui_kittest::Node<'_>) -> String {
    let node = node.accesskit_node();
    node.label().or_else(|| node.value()).unwrap_or_default()
}

/// Stands in for the segment whose text ticks, so ticks can be compared.
const TICKING: &str = "(ticking)";

/// What the bar shows once the harness shows `state`, with `own` as [`TICKING`].
fn shown_around(h: &mut Harness<'_, Fixture>, state: &AppState, own: &str) -> Vec<String> {
    h.state_mut().state = state.clone();
    h.run();
    let shown: Vec<_> = segments(h)
        .into_iter()
        .map(|(label, _)| {
            if label == own {
                TICKING.to_owned()
            } else {
                label
            }
        })
        .collect();
    assert!(!shown.is_empty(), "no status bar beside {own}");
    shown
}

/// At every swept width, each tick shows the same segments, its own included or not.
fn assert_neighbours_hold(ticks: &[(AppState, String)]) {
    let mut h = dashboard(ticks[0].0.clone(), unfiltered, NARROW.x);
    for width in widths() {
        h.set_size(egui::vec2(width, NARROW.y));
        let seen: Vec<_> = ticks
            .iter()
            .map(|(state, own)| shown_around(&mut h, state, own))
            .collect();
        for (shown, (_, own)) in seen.iter().zip(ticks) {
            assert_eq!(shown, &seen[0], "at {width} pt, with {own}");
        }
    }
}

/// The busy demo with `done` comments read, and the progress text it shows.
fn reading(done: u32) -> (AppState, String) {
    let mut state = busy_state();
    let mut progress = state.refresh.progress[0].clone();
    progress.done = done;
    let text = copy::reading_progress(done, progress.total);
    state.refresh.progress = Arc::from([progress]);
    (state, text)
}

/// The crowded demo `secs` before the next refresh, and the countdown it shows.
fn counting_down(secs: u64) -> (AppState, String) {
    let wait = Duration::from_secs(secs);
    let mut state = crowded_state();
    state.refresh.next_due = Some(now().saturating_add(wait));
    // why: the dashboard is drawn focused, so the last minute counts in seconds.
    let text = match countdown(wait, true) {
        Countdown::Minutes(m) => copy::next_in_minutes(m),
        Countdown::Seconds(s) => copy::next_in_seconds(s),
    };
    (state, text)
}

/// The crowded demo last updated `secs` ago, and the text that says so.
fn updated_ago(secs: u64) -> (AppState, String) {
    let at = now().saturating_sub(Duration::from_secs(secs));
    let mut state = crowded_state();
    state.refresh.last_completed = Some(at);
    (state, updated_text(Some(at), now()))
}

#[test]
fn status_bar_keeps_every_count_and_the_active_filter_at_the_smallest_window() {
    let cases: [(&str, Setup); 2] = [
        ("reviewed", |v| v.set_status(StatusFilter::Reviewed)),
        ("changed only", DashboardView::toggle_unread_only),
    ];
    for (case, setup) in cases {
        let h = dashboard(crowded_state(), setup, NARROW.x);
        let query = h.state().dash.view.query.clone();
        let summary = h.state().state.summary;
        let counts = [
            (
                PrStatus::NeedsReview,
                summary.needs_review,
                StatusFilter::NeedsReview,
            ),
            (
                PrStatus::ActiveComments,
                summary.active_comments,
                StatusFilter::ActiveComments,
            ),
            (PrStatus::Reviewed, summary.reviewed, StatusFilter::Reviewed),
        ];
        let mut toggles: Vec<_> = counts
            .into_iter()
            .map(|(status, n, filter)| (copy::status_count(n, status), query.status == filter))
            .collect();
        toggles.push((copy::changed_count(summary.unread), query.unread_only));
        for (label, on) in toggles {
            let toggled = h.get_by_label(&label).accesskit_node().toggled();
            assert_eq!(toggled, Some(Toggled::from(on)), "{case}: {label}");
        }
    }
}

#[test]
fn status_bar_segments_never_overlap_in_a_narrow_window() {
    type State = fn() -> AppState;
    let cases: [(&str, State); 3] = [
        ("demo", demo_state),
        ("crowded", crowded_state),
        ("busy", busy_state),
    ];
    for (case, state) in cases {
        let mut h = dashboard(state(), unfiltered, NARROW.x);
        for width in widths() {
            h.set_size(egui::vec2(width, NARROW.y));
            h.run();
            let found = segments(&h);
            let ordered = found.windows(2).all(|p| p[0].1.max.x <= p[1].1.min.x + 0.5);
            let inside = found.last().is_some_and(|(_, rect)| rect.max.x <= width);
            assert!(ordered && inside, "{case} at {width} pt: {found:?}");
        }
    }
}

#[test]
fn status_bar_keeps_its_neighbours_while_a_refresh_counts_up() {
    assert_neighbours_hold(&[0, 9, 10, 39, 40].map(reading));
}

#[test]
fn status_bar_keeps_its_neighbours_while_the_next_refresh_counts_down() {
    assert_neighbours_hold(&[600, 61, 60, 59, 10, 9].map(counting_down));
}

#[test]
fn status_bar_keeps_its_neighbours_while_the_last_update_ages() {
    assert_neighbours_hold(&[30, 60, 540, 600, 3_540, 7_200].map(updated_ago));
}

#[test]
fn status_bar_shows_the_refresh_state_at_every_width() {
    for (state, own) in [reading(12), counting_down(9)] {
        let mut h = dashboard(state, unfiltered, NARROW.x);
        for width in widths() {
            h.set_size(egui::vec2(width, NARROW.y));
            h.run();
            let found = segments(&h);
            assert!(
                found.iter().any(|(label, _)| *label == own),
                "{own} at {width} pt"
            );
        }
    }
}
