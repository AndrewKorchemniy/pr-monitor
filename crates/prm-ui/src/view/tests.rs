use prm_config::UiState;
use prm_domain::view::{GroupBy, SortKey, StatusFilter};

use super::*;
use crate::testing::{active_comments, draft, needs_review, reviewed, sample_rows};

fn view(group: GroupBy) -> DashboardView {
    let ui = UiState {
        group,
        ..UiState::default()
    };
    DashboardView::new(&ui, sample_rows(), 1, 24.0)
}

fn selected_title(view: &DashboardView) -> Option<String> {
    view.selected_row().map(|r| r.title.to_string())
}

#[test]
fn nav_walks_pr_rows_and_skips_group_headers() {
    let mut v = view(GroupBy::Status);
    v.nav(1);
    let first = v.selected_display().expect("selection");
    assert!(v.display.pr_at(first).is_some());
    let mut seen = vec![v.selected.clone()];
    for _ in 0..10 {
        v.nav(1);
        seen.push(v.selected.clone());
    }
    seen.dedup();
    assert_eq!(seen.len(), 4, "visits each PR once, then stays on the last");
    assert!(v.keyboard_nav);
    v.nav(-100);
    assert_eq!(v.selected_display(), Some(first));
}

#[test]
fn nav_without_a_selection_starts_at_the_matching_end() {
    let mut v = view(GroupBy::None);
    v.nav(-1);
    assert_eq!(v.selected_display(), Some(3));
    let mut v = view(GroupBy::None);
    v.nav_edge(false);
    assert_eq!(v.selected_display(), Some(0));
}

#[test]
fn a_status_filter_keeps_a_matching_selection_or_selects_the_first_row() {
    let mut v = view(GroupBy::None);
    v.select_key(&reviewed());
    v.set_status(StatusFilter::Reviewed);
    assert_eq!(v.selected, Some(reviewed()));
    v.set_status(StatusFilter::ActiveComments);
    assert_eq!(v.selected, Some(active_comments()));
    v.set_status(StatusFilter::ActiveComments);
    assert_eq!(
        v.query.status,
        StatusFilter::All,
        "the same filter again clears it"
    );
}

#[test]
fn filter_text_and_flags_combine_and_clear_together() {
    let mut v = view(GroupBy::None);
    v.set_text("draft");
    assert_eq!(v.model.indices.len(), 1);
    assert_eq!(v.selected, Some(draft()));
    v.toggle_hide_drafts();
    assert_eq!(v.model.indices.len(), 0);
    assert!(v.filters_active());
    v.clear_filters();
    assert!(!v.filters_active());
    assert_eq!(v.model.indices.len(), 4);
}

#[test]
fn sorting_the_same_column_twice_reverses_it() {
    let mut v = view(GroupBy::None);
    v.sort_by(SortKey::Title);
    let first = v.row_at(0).map(|r| r.title.to_string());
    v.sort_by(SortKey::Title);
    assert!(v.sort.descending);
    assert_ne!(v.row_at(0).map(|r| r.title.to_string()), first);
}

#[test]
fn collapsing_the_selected_group_hides_its_rows() {
    let mut v = view(GroupBy::Status);
    v.select_key(&needs_review());
    let before = v.display.rows.len();
    v.set_selected_group_collapsed(true);
    assert!(v.display.rows.len() < before);
    v.toggle_group(0);
    assert_eq!(v.display.rows.len(), before);
}

#[test]
fn new_rows_keep_the_selected_key() {
    let mut v = view(GroupBy::None);
    v.select_key(&reviewed());
    let title = selected_title(&v);
    v.on_rows(sample_rows(), 2);
    assert_eq!(v.rows_generation(), 2);
    assert_eq!(selected_title(&v), title);
}

#[test]
fn ui_state_round_trips_and_column_reset_forgets_widths() {
    let mut v = view(GroupBy::Author);
    v.set_text("fix");
    v.widths.insert("title".to_owned(), 300.0);
    let mut ui = UiState::default();
    v.write_ui_state(&mut ui);
    let restored = DashboardView::new(&ui, sample_rows(), 1, 24.0);
    assert_eq!(restored.group, GroupBy::Author);
    assert_eq!(restored.query.text, "fix");
    assert_eq!(restored.widths.get("title"), Some(&300.0));
    v.reset_columns();
    assert!(v.widths.is_empty());
    assert_eq!(v.table_epoch, 1);
}
