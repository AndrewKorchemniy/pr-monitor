use prm_domain::update::{DisabledReason, UpdateStage};

use super::*;

#[test]
fn updating_is_active_until_disabled_or_ready() {
    let mut runtime = UpdateRuntime::new();
    let cases = [
        (UpdateStatus::Idle, true),
        (UpdateStatus::Checking, true),
        (UpdateStatus::Failed(UpdateStage::Apply), true),
        (UpdateStatus::Ready("0.3.0".into()), false),
        (UpdateStatus::Disabled(DisabledReason::NotInstalled), false),
    ];
    for (status, expected) in cases {
        runtime.status = status.clone();
        assert_eq!(runtime.is_active(), expected, "{status:?}");
    }
}
