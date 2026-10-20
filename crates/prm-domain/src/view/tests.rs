use super::*;
use crate::{
    classify::PrStatus,
    model::PrRecord,
    testing::{BASE_TIME_SECS, PrBuilder, ThreadBuilder, key, person, record, rows, ts},
};

const REVIEWER: u128 = 2;

fn needs_review(n: i32) -> PrRecord {
    record(
        PrBuilder::new(key("contoso", 1, n))
            .author(person(1, "Ann Bell"))
            .build(),
        vec![],
    )
}

fn reviewed(n: i32) -> PrRecord {
    record(
        PrBuilder::new(key("contoso", 1, n))
            .author(person(3, "Cal Diaz"))
            .reviewer(person(REVIEWER, "Rev"), 10)
            .build(),
        vec![],
    )
}

fn active(n: i32) -> PrRecord {
    record(
        PrBuilder::new(key("contoso", 2, n))
            .author(person(1, "Ann Bell"))
            .title("Fix paging")
            .build(),
        vec![
            ThreadBuilder::human(1)
                .comment(person(REVIEWER, "Rev"))
                .build(),
        ],
    )
}

fn sample() -> Vec<PrRow> {
    rows(&[reviewed(1), needs_review(2), active(3)], &[REVIEWER])
}

fn numbers(rows: &[PrRow], view: &ViewModel) -> Vec<i32> {
    view.indices
        .iter()
        .map(|i| rows[*i as usize].key.number.0)
        .collect()
}

fn view_of(rows: &[PrRow], text: &str) -> ViewModel {
    let query = ViewQuery {
        text: text.into(),
        ..ViewQuery::default()
    };
    build_view(rows, &query, Sort::default(), GroupBy::None)
}

#[test]
fn build_rows_unread_and_haystack() {
    let rows = sample();
    assert!(rows.iter().all(|r| r.unread));
    let active = rows.iter().find(|r| r.key.number.0 == 3).unwrap();
    assert!(active.haystack.contains("!3"));
    assert!(active.haystack.contains("fix paging"));
    assert!(active.haystack.contains("contoso"));
    assert_eq!(&*active.comments_text, "1/1");
    assert_eq!(active.threads_preview.len(), 1);
}

#[test]
fn build_rows_default_order_is_attention_rank() {
    let rows = sample();
    let statuses: Vec<_> = rows.iter().map(|r| r.status).collect();
    assert_eq!(
        statuses,
        vec![
            PrStatus::NeedsReview,
            PrStatus::ActiveComments,
            PrStatus::Reviewed
        ]
    );
}

#[test]
fn filter_terms_and() {
    let rows = sample();
    assert_eq!(numbers(&rows, &view_of(&rows, "fix paging")), vec![3]);
    assert_eq!(
        numbers(&rows, &view_of(&rows, "fix nothing")),
        Vec::<i32>::new()
    );
}

#[test]
fn filter_number_bang_hash() {
    let rows = sample();
    assert_eq!(numbers(&rows, &view_of(&rows, "!2")), vec![2]);
    assert_eq!(numbers(&rows, &view_of(&rows, "#1")), vec![1]);
}

#[test]
fn filter_prefixes_author_repo_org_is() {
    let rows = sample();
    assert_eq!(numbers(&rows, &view_of(&rows, "@cal")), vec![1]);
    assert_eq!(numbers(&rows, &view_of(&rows, "org:contoso")).len(), 3);
    assert_eq!(
        numbers(&rows, &view_of(&rows, "is:draft")),
        Vec::<i32>::new()
    );
    assert_eq!(numbers(&rows, &view_of(&rows, "is:unread")).len(), 3);
}

#[test]
fn status_filter() {
    let rows = sample();
    let query = ViewQuery {
        status: StatusFilter::Reviewed,
        ..ViewQuery::default()
    };
    let view = build_view(&rows, &query, Sort::default(), GroupBy::None);
    assert_eq!(numbers(&rows, &view), vec![1]);
    assert_eq!(view.counts.total, 1);
}

#[test]
fn sort_each_key_stable_tiebreak() {
    let rows = sample();
    let by = |key, descending| {
        let view = build_view(
            &rows,
            &ViewQuery::default(),
            Sort { key, descending },
            GroupBy::None,
        );
        numbers(&rows, &view)
    };
    assert_eq!(by(SortKey::Number, false), vec![1, 2, 3]);
    assert_eq!(by(SortKey::Number, true), vec![3, 2, 1]);
    assert_eq!(by(SortKey::Author, false), vec![2, 3, 1]);
    assert_eq!(by(SortKey::Comments, false), vec![3, 2, 1]);
}

#[test]
fn group_status_order() {
    let rows = sample();
    let view = build_view(
        &rows,
        &ViewQuery::default(),
        Sort::default(),
        GroupBy::Status,
    );
    let labels: Vec<&str> = view.groups.iter().map(|g| &*g.label).collect();
    assert_eq!(labels, vec!["Needs review", "Active comments", "Reviewed"]);
    assert!(view.groups.iter().all(|g| g.len == 1));
}

#[test]
fn group_text_alpha() {
    let rows = sample();
    let view = build_view(
        &rows,
        &ViewQuery::default(),
        Sort::default(),
        GroupBy::Author,
    );
    let labels: Vec<&str> = view.groups.iter().map(|g| &*g.label).collect();
    assert_eq!(labels, vec!["Ann Bell", "Cal Diaz"]);
    assert_eq!(view.groups[0].len, 2);
}

#[test]
fn summary_counts() {
    let summary = summarize(&sample());
    assert_eq!(
        summary,
        StatusSummary {
            total: 3,
            needs_review: 1,
            active_comments: 1,
            reviewed: 1,
            drafts: 0,
            unread: 3,
            stale: 0,
        }
    );
    assert_eq!(tray_badge(&summary, false, false), TrayBadge::Attention);
    assert_eq!(tray_badge(&summary, true, false), TrayBadge::Error);
    assert_eq!(
        tray_badge(&StatusSummary::default(), false, true),
        TrayBadge::Syncing
    );
    assert_eq!(
        tray_badge(&StatusSummary::default(), false, false),
        TrayBadge::Idle
    );
}

#[test]
fn tray_tooltip_le_127_utf16_with_emoji() {
    let summary = StatusSummary {
        needs_review: 3,
        active_comments: 2,
        unread: 4,
        ..StatusSummary::default()
    };
    let tip = tray_tooltip(&summary, true);
    assert_eq!(
        tip,
        "PR Monitor · 3 need review · 2 active comments · 4 changed · sign-in needed"
    );
    let long = summary::MAX_TOOLTIP_UTF16;
    assert!(
        text::truncate_utf16(&"😀".repeat(100), long)
            .encode_utf16()
            .count()
            <= long
    );
}

#[test]
fn age_label_table() {
    let now = ts(BASE_TIME_SECS + 10 * 365 * 86_400);
    for (ago_secs, text, bucket) in [
        (30, "now", AgeBucket::Fresh),
        (5 * 60, "5m", AgeBucket::Fresh),
        (3 * 3_600, "3h", AgeBucket::Today),
        (2 * 86_400, "2d", AgeBucket::Week),
        (21 * 86_400, "3w", AgeBucket::Older),
        (120 * 86_400, "4mo", AgeBucket::Older),
        (400 * 86_400, "1y", AgeBucket::Older),
    ] {
        let label = age_label(now, ts(BASE_TIME_SECS + 10 * 365 * 86_400 - ago_secs));
        assert_eq!((&*label.text, label.bucket), (text, bucket), "{ago_secs}s");
    }
}

#[test]
fn age_cache_updates_once_per_minute() {
    let rows = sample();
    let mut cache = AgeCache::new();
    let start = ts(BASE_TIME_SECS + 600);
    assert!(cache.update(&rows, 1, start));
    assert!(!cache.update(
        &rows,
        1,
        start.saturating_add(std::time::Duration::from_secs(1))
    ));
    assert!(cache.update(&rows, 2, start));
    assert!(cache.update(
        &rows,
        2,
        start.saturating_add(std::time::Duration::from_secs(60))
    ));
    assert!(cache.get(0).is_some());
}
