//! Self-updating: the schedule, the download and the restart, against a fake updater.

use std::{sync::Arc, time::Duration};

use prm_domain::{
    testing::{FakeUpdater, UpdaterCall},
    update::{ApplyMode, DisabledReason, UpdateStage, UpdateStatus},
};

use crate::{
    command::Command,
    event::AppEvent,
    testing::{HarnessSetup, ServiceHarness},
    updates::{CHECK_INTERVAL, CHECK_TIMEOUT, FIRST_CHECK, RETRY_AFTER_FAILURE},
};

const VERSION: &str = "0.3.0";

/// An installed app, with automatic checks on or off.
fn setup(auto: bool) -> HarnessSetup {
    let mut setup = HarnessSetup::default();
    setup.config.updates.auto = auto;
    setup.updater = Arc::new(FakeUpdater::default());
    setup
}

async fn status(h: &mut ServiceHarness, what: &str, expected: UpdateStatus) {
    h.wait_for(what, move |s| s.update == expected).await;
}

#[tokio::test(start_paused = true)]
async fn the_first_check_comes_a_minute_after_start() {
    let mut h = ServiceHarness::start(setup(true));

    h.advance(FIRST_CHECK.saturating_sub(Duration::from_secs(1)))
        .await;
    assert_eq!(h.updater.checks(), 0, "too early");
    assert_eq!(h.snapshot().update, UpdateStatus::Idle);
    h.advance(Duration::from_secs(2)).await;
    status(&mut h, "an up-to-date result", UpdateStatus::UpToDate).await;

    assert_eq!(h.updater.checks(), 1);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn checks_repeat_every_six_hours() {
    let mut h = ServiceHarness::start(setup(true));
    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(&mut h, "the first check", UpdateStatus::UpToDate).await;

    h.advance(CHECK_INTERVAL.saturating_sub(Duration::from_secs(1)))
        .await;
    assert_eq!(h.updater.checks(), 1, "not yet");
    h.advance(Duration::from_secs(2)).await;
    h.updater.wait_for_calls(2);

    assert_eq!(h.updater.checks(), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn with_automatic_checks_off_only_check_now_looks() {
    let mut h = ServiceHarness::start(setup(false));
    h.advance(CHECK_INTERVAL * 2).await;
    assert_eq!(h.updater.checks(), 0);
    assert_eq!(h.snapshot().update, UpdateStatus::Idle);

    h.send(Command::CheckForUpdates);
    status(&mut h, "an up-to-date result", UpdateStatus::UpToDate).await;

    assert_eq!(h.updater.checks(), 1);
    h.advance(CHECK_INTERVAL * 2).await;
    assert_eq!(h.updater.checks(), 1, "no schedule follows a manual check");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_found_update_downloads_and_waits_for_a_restart() {
    let mut h = ServiceHarness::start(setup(true));
    h.updater.offer(VERSION);

    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(
        &mut h,
        "a ready update",
        UpdateStatus::Ready(VERSION.into()),
    )
    .await;

    assert_eq!(
        h.updater.calls(),
        [UpdaterCall::Check, UpdaterCall::Download(VERSION.into())]
    );
    h.advance(CHECK_INTERVAL * 2).await;
    assert_eq!(h.updater.checks(), 1, "nothing more to look for");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_failed_check_is_retried_after_an_hour() {
    let mut h = ServiceHarness::start(setup(true));
    h.updater.fail_next_check();

    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(
        &mut h,
        "a failed check",
        UpdateStatus::Failed(UpdateStage::Check),
    )
    .await;
    h.advance(RETRY_AFTER_FAILURE.saturating_sub(Duration::from_secs(1)))
        .await;
    assert_eq!(h.updater.checks(), 1, "not yet");
    h.advance(Duration::from_secs(2)).await;

    status(&mut h, "the retry", UpdateStatus::UpToDate).await;
    assert_eq!(h.updater.checks(), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_failed_download_names_the_download() {
    let mut h = ServiceHarness::start(setup(true));
    h.updater.offer(VERSION);
    h.updater.fail_next_download();

    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;

    let failed = UpdateStatus::Failed(UpdateStage::Download);
    status(&mut h, "a failed download", failed).await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_check_that_never_returns_fails_after_the_timeout() {
    let mut h = ServiceHarness::start(setup(true));
    h.updater.hold();
    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(&mut h, "a running check", UpdateStatus::Checking).await;
    h.updater.wait_for_calls(1);

    h.advance(CHECK_TIMEOUT + Duration::from_secs(1)).await;

    status(
        &mut h,
        "a failed check",
        UpdateStatus::Failed(UpdateStage::Check),
    )
    .await;
    h.updater.release();
    h.stop().await;
}

/// A check that timed out still occupies its blocking thread; retrying while it hangs would
/// leak a thread per retry until the small pool starves the other ports.
#[tokio::test(start_paused = true)]
async fn a_hung_check_is_not_retried_until_its_thread_returns() {
    let mut h = ServiceHarness::start(setup(true));
    h.updater.hold();
    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(&mut h, "a running check", UpdateStatus::Checking).await;
    h.updater.wait_for_calls(1);
    h.advance(CHECK_TIMEOUT + Duration::from_secs(1)).await;
    let failed = UpdateStatus::Failed(UpdateStage::Check);
    status(&mut h, "a failed check", failed).await;

    h.advance(RETRY_AFTER_FAILURE + Duration::from_secs(1))
        .await;
    assert_eq!(h.updater.checks(), 1, "the first call is still running");
    h.send(Command::CheckForUpdates);
    h.advance(Duration::from_secs(1)).await;
    assert_eq!(h.updater.checks(), 1, "Check now waits for it too");

    h.updater.release();
    // why: the released thread drops its guard a moment after `release`. A retry that fires
    // before then waits another hour, so keep advancing until a check starts.
    for _ in 0..5 {
        h.advance(RETRY_AFTER_FAILURE + Duration::from_secs(1))
            .await;
        if h.snapshot().update == UpdateStatus::Checking {
            break;
        }
    }
    status(&mut h, "the next check", UpdateStatus::UpToDate).await;
    assert_eq!(h.updater.checks(), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_package_downloaded_earlier_shows_ready_without_a_check() {
    let setup = setup(false);
    setup.updater.set_pending(Some(VERSION));
    let mut h = ServiceHarness::start(setup);

    status(
        &mut h,
        "a ready update",
        UpdateStatus::Ready(VERSION.into()),
    )
    .await;
    h.send(Command::CheckForUpdates);
    h.advance(CHECK_INTERVAL).await;

    assert_eq!(h.updater.checks(), 0);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_app_that_cannot_update_never_calls_the_updater() {
    let setup = setup(true);
    setup
        .updater
        .set_availability(Err(DisabledReason::IsolatedHome));
    let mut h = ServiceHarness::start(setup);
    let disabled = UpdateStatus::Disabled(DisabledReason::IsolatedHome);
    status(&mut h, "a disabled status", disabled).await;

    h.send(Command::CheckForUpdates);
    h.send(Command::ApplyUpdate);
    h.advance(CHECK_INTERVAL).await;

    assert_eq!(h.updater.calls(), []);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn applying_hands_over_to_the_updater_then_asks_the_ui_to_quit() {
    let setup = setup(true);
    setup.updater.set_pending(Some(VERSION));
    let mut h = ServiceHarness::start(setup);
    status(
        &mut h,
        "a ready update",
        UpdateStatus::Ready(VERSION.into()),
    )
    .await;

    h.send(Command::ApplyUpdate);
    let events = h.events.clone();
    h.wait_for("the quit request", move |_| {
        events.events().contains(&AppEvent::QuitForUpdate)
    })
    .await;

    assert_eq!(
        h.updater.calls(),
        [UpdaterCall::Apply(Vec::new(), ApplyMode::Interactive)]
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_failed_apply_keeps_the_app_running() {
    let setup = setup(true);
    setup.updater.set_pending(Some(VERSION));
    setup.updater.fail_apply();
    let mut h = ServiceHarness::start(setup);
    status(
        &mut h,
        "a ready update",
        UpdateStatus::Ready(VERSION.into()),
    )
    .await;

    h.send(Command::ApplyUpdate);

    status(
        &mut h,
        "a failed apply",
        UpdateStatus::Failed(UpdateStage::Apply),
    )
    .await;
    assert!(!h.events().contains(&AppEvent::QuitForUpdate));
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn applying_without_a_ready_update_does_nothing() {
    let h = ServiceHarness::start(setup(true));

    h.send(Command::ApplyUpdate);
    h.advance(Duration::from_secs(1)).await;

    assert_eq!(h.updater.calls(), []);
    assert_eq!(h.events(), Vec::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn turning_automatic_checks_on_or_off_moves_the_schedule() {
    let mut h = ServiceHarness::start(setup(false));
    h.advance(FIRST_CHECK * 2).await;
    assert_eq!(h.updater.checks(), 0);

    let mut config = (*h.snapshot().config).clone();
    config.updates.auto = true;
    h.send(Command::ApplyConfig(Box::new(config.clone())));
    h.advance(FIRST_CHECK + Duration::from_secs(1)).await;
    status(&mut h, "a check after turning on", UpdateStatus::UpToDate).await;
    assert_eq!(h.updater.checks(), 1);

    config.updates.auto = false;
    h.send(Command::ApplyConfig(Box::new(config)));
    h.advance(CHECK_INTERVAL * 2).await;

    assert_eq!(h.updater.checks(), 1, "turned off again");
    h.stop().await;
}
