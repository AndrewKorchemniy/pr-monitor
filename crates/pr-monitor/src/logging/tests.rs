use tracing::Level;

use super::*;

/// Whether `targets` lets an event at `level` from `target` through.
fn allows(targets: &Targets, target: &str, level: Level) -> bool {
    targets.would_enable(target, &level)
}

#[test]
fn the_config_level_applies_to_our_code_and_noisy_crates_stay_at_warn() {
    let targets = log_targets(None, "debug", false);
    assert!(allows(&targets, "prm_service", Level::DEBUG));
    assert!(!allows(&targets, "wgpu_core", Level::INFO));
    assert!(allows(&targets, "wgpu_core", Level::WARN));
}

#[test]
fn an_invalid_config_level_falls_back_to_info_and_debug_wins() {
    let targets = log_targets(None, "chatty", false);
    assert!(allows(&targets, "prm_ui", Level::INFO));
    assert!(!allows(&targets, "prm_ui", Level::DEBUG));
    assert!(allows(
        &log_targets(None, "error", true),
        "prm_ui",
        Level::DEBUG
    ));
}

#[test]
fn the_environment_filter_is_used_verbatim() {
    let targets = log_targets(Some("wgpu_core=trace,warn"), "info", false);
    assert!(allows(&targets, "wgpu_core", Level::TRACE));
    assert!(!allows(&targets, "prm_ui", Level::INFO));
    let ignored = log_targets(Some("=not a filter="), "info", false);
    assert!(allows(&ignored, "prm_ui", Level::INFO));
}
