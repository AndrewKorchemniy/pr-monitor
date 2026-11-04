use super::*;

fn view(status: &UpdateStatus) -> View {
    describe(status)
}

#[test]
fn each_status_has_a_sentence_and_the_actions_that_fit() {
    let cases = [
        (UpdateStatus::Idle, copy::UPDATE_NOT_CHECKED, RESTING),
        (UpdateStatus::Checking, copy::UPDATE_CHECKING, NONE),
        (UpdateStatus::UpToDate, copy::UPDATE_UP_TO_DATE, RESTING),
        (
            UpdateStatus::Failed(UpdateStage::Check),
            copy::UPDATE_CHECK_FAILED,
            FAILED,
        ),
        (
            UpdateStatus::Failed(UpdateStage::Download),
            copy::UPDATE_DOWNLOAD_FAILED,
            FAILED,
        ),
        (
            UpdateStatus::Failed(UpdateStage::Apply),
            copy::UPDATE_APPLY_FAILED,
            FAILED,
        ),
        (
            UpdateStatus::Disabled(DisabledReason::NotInstalled),
            copy::UPDATE_NOT_INSTALLED,
            NONE,
        ),
        (
            UpdateStatus::Disabled(DisabledReason::IsolatedHome),
            copy::UPDATE_ISOLATED_HOME,
            NONE,
        ),
    ];
    for (status, text, actions) in cases {
        let shown = view(&status);
        assert_eq!(shown.text, text, "{status:?}");
        assert_eq!(shown.actions, actions, "{status:?}");
    }
}

#[test]
fn a_download_shows_its_percent_and_a_ready_update_offers_the_restart() {
    let downloading = view(&UpdateStatus::downloading(42));
    assert_eq!(downloading.text, "Downloading update… 42%");
    assert_eq!(downloading.actions, NONE);

    let ready = view(&UpdateStatus::Ready("0.3.0".into()));
    assert_eq!(ready.text, "Version 0.3.0 is ready");
    assert_eq!(ready.actions, [Action::Restart]);
}

#[test]
fn a_failure_offers_a_retry_and_the_releases_page_and_nothing_pops_up() {
    let failed = view(&UpdateStatus::Failed(UpdateStage::Check));

    assert_eq!(failed.actions, [Action::CheckNow, Action::Releases]);
    assert_eq!(Action::Releases.command().map(|_| ()), None);
    assert!(matches!(Action::Releases.link(), Some(Link::Releases)));
}

#[test]
fn check_now_and_restart_send_their_commands() {
    assert!(matches!(
        Action::CheckNow.command(),
        Some(Command::CheckForUpdates)
    ));
    assert!(matches!(
        Action::Restart.command(),
        Some(Command::ApplyUpdate)
    ));
    assert!(Action::CheckNow.link().is_none() && Action::Restart.link().is_none());
}
