use prm_config::{ConfigIssue, PersistedState, Severity};

use super::*;

#[test]
fn autostart_waits_at_least_ten_seconds_and_zero_means_no_delay() {
    assert_eq!(
        effective_startup_delay(true, 0),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        effective_startup_delay(true, 30),
        Some(Duration::from_secs(30))
    );
    assert_eq!(effective_startup_delay(false, 0), None);
    assert_eq!(
        effective_startup_delay(false, 5),
        Some(Duration::from_secs(5))
    );
}

#[test]
fn start_hidden_unless_a_link_asked_to_show_something() {
    let mut config = Config::default();
    let minimized = Args {
        minimized: true,
        ..Args::default()
    };
    assert!(start_hidden(&minimized, &config));
    let linked = Args {
        activate: Some("prmonitor://show".to_owned()),
        ..minimized
    };
    assert!(!start_hidden(&linked, &config));
    config.ui.start_minimized = true;
    assert!(start_hidden(&Args::default(), &config));
}

#[test]
fn initial_data_carries_config_and_state_issues() {
    let issue = |text: &str| ConfigIssue {
        severity: Severity::Warning,
        path: text.into(),
        message: text.into(),
    };
    let config = ConfigLoad {
        config: Config::default(),
        issues: vec![issue("config")],
        read_only: false,
    };
    let state = StateLoad {
        state: PersistedState::default(),
        issue: Some(issue("state")),
    };
    let data = initial_data(
        config,
        state,
        HashMap::new(),
        &Args {
            minimized: true,
            ..Args::default()
        },
    );
    assert_eq!(data.config_issues.len(), 2);
    assert_eq!(data.startup_delay, Some(Duration::from_secs(10)));
    assert_eq!(data.rng_seed, None);
}
