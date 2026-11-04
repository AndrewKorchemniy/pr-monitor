use super::*;

fn every_status() -> [UpdateStatus; 9] {
    [
        UpdateStatus::Idle,
        UpdateStatus::Checking,
        UpdateStatus::UpToDate,
        UpdateStatus::downloading(40),
        UpdateStatus::Ready("0.3.0".into()),
        UpdateStatus::Failed(UpdateStage::Check),
        UpdateStatus::Failed(UpdateStage::Apply),
        UpdateStatus::Disabled(DisabledReason::NotInstalled),
        UpdateStatus::Disabled(DisabledReason::IsolatedHome),
    ]
}

#[test]
fn a_check_starts_only_from_a_resting_status() {
    let allowed = [true, false, true, false, false, true, true, false, false];
    for (status, expected) in every_status().into_iter().zip(allowed) {
        assert_eq!(status.can_check(), expected, "{status:?}");
    }
}

#[test]
fn download_progress_is_clamped_to_one_hundred() {
    assert_eq!(UpdateStatus::downloading(0), UpdateStatus::Downloading(0));
    assert_eq!(
        UpdateStatus::downloading(100),
        UpdateStatus::Downloading(100)
    );
    assert_eq!(
        UpdateStatus::downloading(250),
        UpdateStatus::Downloading(100)
    );
}

#[test]
fn only_ready_names_a_version() {
    for status in every_status() {
        let expected = matches!(status, UpdateStatus::Ready(_)).then_some("0.3.0");
        assert_eq!(status.ready_version(), expected, "{status:?}");
    }
}

#[test]
fn a_new_run_starts_idle() {
    assert_eq!(UpdateStatus::default(), UpdateStatus::Idle);
}
