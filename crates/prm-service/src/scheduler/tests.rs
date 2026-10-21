use std::time::Duration;

use prm_domain::testing::{org, ts};

use super::*;

const MINUTE: Duration = Duration::from_mins(1);

fn policy(jitter_pct: u8) -> Policy {
    Policy {
        interval: 5 * MINUTE,
        jitter_pct,
    }
}

fn at(mono: Instant, wall_secs: i64) -> Moment {
    Moment {
        mono,
        wall: ts(wall_secs),
    }
}

fn delay(schedule: &Schedule, now: Moment) -> Duration {
    schedule
        .due_mono()
        .map_or(Duration::MAX, |due| due - now.mono)
}

#[test]
fn jitter_within_bounds() {
    let mut rng = fastrand::Rng::with_seed(1);
    let base = Duration::from_secs(100);
    for _ in 0..1_000 {
        let value = jittered(base, 10, &mut rng);
        assert!(
            (Duration::from_secs(90)..=Duration::from_secs(110)).contains(&value),
            "{value:?}"
        );
    }
    assert_eq!(jittered(base, 0, &mut rng), base);
}

#[test]
fn adjust_multiplier_hysteresis() {
    let cases = [
        (1, Some(0.1), 2),
        (4, Some(0.1), 8),
        (8, Some(0.1), 8),
        (4, Some(0.3), 4),
        (4, Some(0.5), 2),
        (1, Some(0.9), 1),
        (2, None, 2),
    ];
    for (current, headroom, expected) in cases {
        assert_eq!(
            adjust_multiplier(current, headroom),
            expected,
            "{current} {headroom:?}"
        );
    }
}

#[test]
fn after_success_resets_failures_and_uses_interval() {
    let now = at(Instant::now(), 1_000);
    let mut schedule = Schedule {
        failures: 3,
        ..Schedule::default()
    };
    let mut rng = fastrand::Rng::with_seed(1);
    after_success(policy(0), &mut schedule, now, None, &mut rng);
    assert_eq!(schedule.failures, 0);
    assert_eq!(delay(&schedule, now), 5 * MINUTE);
    assert_eq!(schedule.due_wall(), Some(ts(1_300)));
}

#[test]
fn after_failure_exponential_capped_by_interval() {
    let now = at(Instant::now(), 0);
    let mut schedule = Schedule::default();
    let mut rng = fastrand::Rng::with_seed(1);
    let mut delays = Vec::new();
    for _ in 0..5 {
        after_failure(policy(0), &mut schedule, now, FailureKind::Other, &mut rng);
        delays.push(delay(&schedule, now));
    }
    assert_eq!(
        delays,
        [MINUTE, 2 * MINUTE, 4 * MINUTE, 5 * MINUTE, 5 * MINUTE]
    );
}

#[test]
fn after_rate_limit_respects_retry_after() {
    let now = at(Instant::now(), 0);
    let mut rng = fastrand::Rng::with_seed(1);
    let mut schedule = Schedule::default();
    let long = FailureKind::RateLimited {
        retry_after: Some(Duration::from_mins(30)),
    };
    after_failure(policy(0), &mut schedule, now, long, &mut rng);
    assert_eq!(schedule.multiplier, 2);
    assert_eq!(delay(&schedule, now), Duration::from_mins(30));

    let mut schedule = Schedule::default();
    let unknown = FailureKind::RateLimited { retry_after: None };
    after_failure(policy(0), &mut schedule, now, unknown, &mut rng);
    assert_eq!(
        delay(&schedule, now),
        10 * MINUTE,
        "interval × 2 beats 30 s"
    );
}

#[test]
fn failure_kind_of_rate_limited() {
    let error = AdoError::RateLimited {
        retry_after: Some(Duration::from_secs(9)),
    };
    assert_eq!(
        FailureKind::of(&error),
        FailureKind::RateLimited {
            retry_after: Some(Duration::from_secs(9))
        }
    );
    assert_eq!(FailureKind::of(&AdoError::Timeout), FailureKind::Other);
}

#[test]
fn due_orgs_coalesce_window() {
    let now = Instant::now();
    let due_in = |secs: u64| {
        let mut schedule = Schedule::default();
        schedule.due_at(at(now, 0), Duration::from_secs(secs));
        schedule
    };
    let names = ["soon", "later", "paused", "busy", "unscheduled"].map(org);
    let soon = due_in(4);
    let later = due_in(6);
    let paused = Schedule {
        paused: Some(PauseReason::Auth),
        ..due_in(0)
    };
    let busy = due_in(0);
    let unscheduled = Schedule::default();
    let checks = [
        (&names[0], &soon, false),
        (&names[1], &later, false),
        (&names[2], &paused, false),
        (&names[3], &busy, true),
        (&names[4], &unscheduled, false),
    ];
    let due = due_orgs(
        checks.map(|(org, schedule, in_flight)| DueCheck {
            org,
            schedule,
            in_flight,
        }),
        now,
    );
    assert_eq!(due, vec![org("soon")]);
}

#[test]
fn detect_resume_threshold() {
    let start = Instant::now();
    let before = at(start, 0);
    let deadline = start + Duration::from_secs(60);
    let after = |wall: i64| at(deadline, wall);
    assert!(!detect_resume(before, after(60 + 89), deadline));
    assert!(detect_resume(before, after(60 + 91), deadline));
    assert!(!detect_resume(before, after(60), deadline));
}

/// Windows' monotonic clock keeps counting in suspend, so both clocks jump together; resume
/// used to go unnoticed there and refreshes fired before the network was back.
#[test]
fn detect_resume_timer_fired_late() {
    let start = Instant::now();
    let before = at(start, 0);
    let deadline = start + Duration::from_secs(60);
    let late = |secs: u64| {
        let mono = deadline + Duration::from_secs(secs);
        let wall = i64::try_from((mono - start).as_secs()).unwrap();
        at(mono, wall)
    };
    assert!(!detect_resume(before, late(89), deadline));
    assert!(detect_resume(before, late(91), deadline));
    assert!(detect_resume(before, late(2 * 3_600), deadline));
}

#[test]
fn schedule_clear_due_clears_both_clocks() {
    let now = at(Instant::now(), 0);
    let mut schedule = Schedule::default();
    schedule.due_at(now, Duration::from_secs(60));
    schedule.clear_due();
    assert_eq!(schedule.due_mono(), None);
    assert_eq!(schedule.due_wall(), None);
}

#[test]
fn policy_probe_interval_at_least_refresh_interval() {
    assert_eq!(policy(0).probe_interval(), AUTH_PROBE_INTERVAL);
    let slow = Policy {
        interval: Duration::from_mins(30),
        jitter_pct: 0,
    };
    assert_eq!(slow.probe_interval(), Duration::from_mins(30));
}

#[test]
fn policy_from_config_minutes() {
    let config = RefreshConfig {
        interval_minutes: 7,
        ..RefreshConfig::default()
    };
    assert_eq!(Policy::from_config(&config).interval, 7 * MINUTE);
}

#[test]
fn moment_converts_between_clocks() {
    let now = at(Instant::now(), 1_000);
    let later = now.mono + Duration::from_secs(30);
    assert_eq!(now.wall_of(later), ts(1_030));
    assert_eq!(now.mono_of(ts(1_030)), later);
}

#[test]
fn due_no_later_than_only_pulls_in() {
    let now = at(Instant::now(), 0);
    let mut schedule = Schedule::default();
    schedule.due_at(now, Duration::from_secs(60));
    schedule.due_no_later_than(now, Duration::from_secs(120));
    assert_eq!(delay(&schedule, now), Duration::from_secs(60));
    schedule.due_no_later_than(now, Duration::from_secs(10));
    assert_eq!(delay(&schedule, now), Duration::from_secs(10));
}
