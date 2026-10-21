//! Rate limits, timeouts, per-org isolation, partial failures, closed PRs and panics.

use std::time::Duration;

use prm_ado::{AdoError, RateSnapshot, testing::ApiCall};
use prm_config::{
    ConfigError,
    testing::{MemoryRepo, Saved},
};
use prm_domain::{
    diff::ChangeKind,
    model::PrState,
    testing::{ThreadBuilder, org, project},
    tombstone::CloseOutcome,
};
use tokio::time::Instant;

use super::support::{
    CONTOSO, FABRIKAM, change_kinds, fake_org, list_calls, pr, pr_in, refreshed, rev, settled,
    setup,
};
use crate::{
    command::Command,
    consts::PERSIST_DEBOUNCE,
    event::AppEvent,
    state::{AppState, OrgHealth, PauseReason},
    testing::{HarnessSetup, ServiceHarness},
    text,
};

fn state_saves(repo: &MemoryRepo) -> usize {
    repo.saved()
        .iter()
        .filter(|s| matches!(s, Saved::State(_)))
        .count()
}

fn rate(remaining: f64) -> RateSnapshot {
    RateSnapshot {
        limit: Some(100.0),
        remaining: Some(remaining),
        ..RateSnapshot::default()
    }
}

#[tokio::test(start_paused = true)]
async fn rate_headroom_low_doubles_interval_and_recovers() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start.orgs[0].1.rate = rate(10.0);
    let mut h = ServiceHarness::start(start);
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert!(state.refresh.slowed);
    assert_eq!(state.org(&org(CONTOSO)).unwrap().rate.multiplier, 2);
    let first = Instant::now();
    h.api(&org(CONTOSO)).update(|o| o.rate = rate(90.0));

    let state = refreshed(&mut h, CONTOSO, 2).await;

    let gap = first.elapsed();
    assert!(
        (Duration::from_mins(9)..=Duration::from_mins(11)).contains(&gap),
        "{gap:?}"
    );
    assert_eq!(state.org(&org(CONTOSO)).unwrap().rate.multiplier, 1);
    assert!(!state.refresh.slowed);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn rate_limited_failure_pauses_with_reason() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    h.api(&org(CONTOSO)).fail(
        ApiCall::ActivePullRequests,
        AdoError::RateLimited {
            retry_after: Some(Duration::from_mins(20)),
        },
    );

    let state = refreshed(&mut h, CONTOSO, 1).await;

    let status = state.org(&org(CONTOSO)).unwrap();
    assert_eq!(
        status.health,
        OrgHealth::Paused {
            reason: PauseReason::RateLimited
        }
    );
    assert_eq!(status.rate.multiplier, 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn slow_org_times_out_at_120s() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    h.api(&org(CONTOSO))
        .set_delay(Some(Duration::from_secs(600)));
    let began = Instant::now();

    let state = h.wait_for("timeout", |s| settled(s, &org(CONTOSO))).await;

    assert_eq!(began.elapsed(), Duration::from_secs(120));
    let OrgHealth::Failed { error, .. } = &state.org(&org(CONTOSO)).unwrap().health else {
        panic!("expected Failed");
    };
    assert_eq!(error.title.as_ref(), text::REFRESH_TIMED_OUT);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn one_org_failure_does_not_block_other() {
    let mut h = ServiceHarness::start(setup(vec![
        (CONTOSO, vec![pr(CONTOSO, 1)]),
        (FABRIKAM, vec![pr(FABRIKAM, 2)]),
    ]));
    h.api(&org(CONTOSO))
        .fail(ApiCall::Projects, AdoError::Network("down".into()));

    h.wait_for("both settled", |s| {
        settled(s, &org(CONTOSO)) && settled(s, &org(FABRIKAM))
    })
    .await;
    let state = refreshed(&mut h, FABRIKAM, 1).await;

    assert!(matches!(
        state.org(&org(CONTOSO)).unwrap().health,
        OrgHealth::Failed { .. }
    ));
    assert_eq!(state.org(&org(FABRIKAM)).unwrap().health, OrgHealth::Ok);
    assert_eq!(state.rows.len(), 1);
    assert_eq!(state.rows[0].key.org, org(FABRIKAM));
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn partial_project_failure_keeps_prs_no_closed() {
    let prs = vec![pr_in(CONTOSO, 1, 1), pr_in(CONTOSO, 2, 2)];
    let mut h = ServiceHarness::start(HarnessSetup {
        orgs: vec![(org(CONTOSO), fake_org(2, prs))],
        ..setup(vec![(CONTOSO, Vec::new())])
    });
    let state = refreshed(&mut h, CONTOSO, 2).await;
    assert_eq!(state.rows.len(), 2);
    h.api(&org(CONTOSO)).fail_project(
        project(2),
        AdoError::Server {
            status: 503,
            message: None,
            activity: None,
        },
    );

    h.send(Command::RefreshNow);
    let state = h
        .wait_for("degraded", |s| {
            matches!(
                s.org(&org(CONTOSO)).unwrap().health,
                OrgHealth::Degraded { .. }
            )
        })
        .await;

    assert_eq!(state.rows.len(), 2);
    let stale: Vec<bool> = state.rows.iter().map(|r| r.stale).collect();
    assert_eq!(stale.iter().filter(|s| **s).count(), 1);
    assert_eq!(h.api(&org(CONTOSO)).calls(ApiCall::PullRequest), 0);
    assert_eq!(change_kinds(&h), Vec::new());
    assert_eq!(state.org(&org(CONTOSO)).unwrap().scope.repos_failed, 1);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn thread_failure_reuses_previous_threads_stale() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    let key = pr(CONTOSO, 1).key;
    let thread = ThreadBuilder::human(1).comment(rev()).build();
    start.orgs[0].1.threads.insert(key.clone(), vec![thread]);
    let mut h = ServiceHarness::start(start);
    let state = refreshed(&mut h, CONTOSO, 1).await;
    assert_eq!(state.rows[0].active_threads, 1);
    h.api(&org(CONTOSO))
        .fail(ApiCall::Threads, AdoError::Network("flaky".into()));

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(state.rows[0].active_threads, 1);
    assert!(state.rows[0].stale);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn all_queries_failed_marks_org_failed() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO)).fail(
        ApiCall::ActivePullRequests,
        AdoError::Server {
            status: 500,
            message: None,
            activity: None,
        },
    );

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert!(matches!(
        state.org(&org(CONTOSO)).unwrap().health,
        OrgHealth::Failed { .. }
    ));
    assert_eq!(state.rows.len(), 1, "last data stays");
    assert!(state.rows[0].stale);
    assert!(state.any_org_broken());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn vanished_resolved_completed_emits_closed_and_tombstone() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let key = pr(CONTOSO, 1).key;
    h.api(&org(CONTOSO))
        .update(|o| o.prs[0].state = PrState::Completed);

    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 2).await;

    assert_eq!(state.rows.len(), 0);
    let closed = ChangeKind::Closed {
        outcome: CloseOutcome::Completed,
    };
    assert_eq!(change_kinds(&h), vec![(key.clone(), closed)]);
    let repo = h.repo.clone();
    let tombstoned = key.clone();
    h.wait_for("tombstone saved", move |_| {
        repo.last_state()
            .is_some_and(|s| s.orgs[&org(CONTOSO)].tombstones.contains_key(&tombstoned))
    })
    .await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn reactivated_pr_emits_reactivated() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let api = h.api(&org(CONTOSO));
    api.update(|o| o.prs[0].state = PrState::Abandoned);
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;

    api.update(|o| o.prs[0].state = PrState::Active);
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    let state = refreshed(&mut h, CONTOSO, 3).await;

    assert_eq!(state.rows.len(), 1);
    let kinds: Vec<ChangeKind> = change_kinds(&h).into_iter().map(|(_, k)| k).collect();
    assert_eq!(
        kinds,
        vec![
            ChangeKind::Closed {
                outcome: CloseOutcome::Abandoned
            },
            ChangeKind::Reactivated
        ]
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn job_panic_becomes_org_error() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    h.api(&org(CONTOSO)).panic_on(Some(ApiCall::Projects));

    let state = h.wait_for("failed", |s| settled(s, &org(CONTOSO))).await;

    let OrgHealth::Failed { error, .. } = &state.org(&org(CONTOSO)).unwrap().health else {
        panic!("expected Failed");
    };
    assert_eq!(error.title.as_ref(), text::INTERNAL_ERROR);
    h.api(&org(CONTOSO)).panic_on(None);
    refreshed(&mut h, CONTOSO, 1).await;
    h.stop().await;
}

fn catalog_failed(state: &AppState) -> bool {
    state
        .org(&org(CONTOSO))
        .is_some_and(|o| o.catalog_error.is_some())
}

/// A failed project list load used to show only as a passing message, so the repository
/// picker said it was loading projects forever.
#[tokio::test(start_paused = true)]
async fn catalog_failure_stays_on_the_org_until_a_reload_starts() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let load = || Command::LoadCatalog {
        org: org(CONTOSO),
        force: true,
    };
    h.api(&org(CONTOSO))
        .fail(ApiCall::Projects, AdoError::Network("down".into()));

    h.send(load());
    let failed = h.wait_for("catalog error", catalog_failed).await;
    h.api(&org(CONTOSO)).clear_failures();
    h.send(load());
    let reloaded = h
        .wait_for("catalog error cleared", |s| !catalog_failed(s))
        .await;

    let error = failed
        .org(&org(CONTOSO))
        .and_then(|o| o.catalog_error.clone());
    assert_eq!(
        error.map(|e| e.title.to_string()).as_deref(),
        Some(text::UNREACHABLE)
    );
    assert!(
        reloaded
            .org(&org(CONTOSO))
            .is_some_and(|o| o.catalog.is_some())
    );
    h.stop().await;
}

fn save_banner(state: &AppState) -> bool {
    state
        .banners
        .iter()
        .any(|b| b.id.as_ref() == "state-not-saved")
}

#[tokio::test(start_paused = true)]
async fn persist_failure_banner_shows_until_a_save_succeeds() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let repo = h.repo.clone();
    h.wait_for("catalog saved", move |_| !repo.saved().is_empty())
        .await;
    h.repo.fail_next(ConfigError::Serialize("disk full".into()));

    h.wait_for("save banner", save_banner).await;
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.wait_for("banner cleared", |s| !save_banner(s)).await;

    assert_eq!(list_calls(&h, CONTOSO), 2);
    h.stop().await;
}

/// A failed state save used to wait for the next unrelated change, so a crash in between lost
/// seen marks and could repeat toasts.
#[tokio::test(start_paused = true)]
async fn persist_failure_retried() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let repo = h.repo.clone();
    h.wait_for("first save", move |_| repo.last_state().is_some())
        .await;
    let before = state_saves(&h.repo);
    h.repo.fail_next(ConfigError::Serialize("disk full".into()));

    h.send(Command::MarkAllSeen);
    h.wait_for("save banner", save_banner).await;
    let failed_at = Instant::now();
    let repo = h.repo.clone();
    h.wait_for("retried save", move |_| state_saves(&repo) > before)
        .await;

    assert!(
        failed_at.elapsed() <= PERSIST_DEBOUNCE,
        "{:?}",
        failed_at.elapsed()
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn shutdown_cancels_jobs_persists_and_stops() {
    let h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    h.api(&org(CONTOSO))
        .set_delay(Some(Duration::from_hours(1)));
    h.settle().await;
    let (events, repo) = (h.events.clone(), h.repo.clone());

    h.stop().await;

    assert_eq!(events.events().last(), Some(&AppEvent::Stopped));
    assert!(repo.last_state().is_some());
}
