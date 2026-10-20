use std::time::Duration;

use prm_config::UiState;
use prm_domain::testing::ts;
use prm_service::{AppState, Command};

use super::*;
use crate::testing::link;

fn changed() -> UiState {
    UiState {
        peek_open: true,
        ..UiState::default()
    }
}

fn saves(commands: &[Command]) -> usize {
    commands
        .iter()
        .filter(|c| matches!(c, Command::SaveUiState(_)))
        .count()
}

#[test]
fn an_unchanged_state_is_never_saved() {
    let (link, mut ends) = link(AppState::default());
    let mut sync = UiStateSync::new(UiState::default());
    sync.tick(ts(0), UiState::default(), &link);
    sync.tick(ts(10), UiState::default(), &link);
    assert_eq!(saves(&ends.drain()), 0);
    assert_eq!(sync.deadline(), None);
}

#[test]
fn a_change_is_saved_once_it_has_been_stable_for_a_second() {
    let (link, mut ends) = link(AppState::default());
    let mut sync = UiStateSync::new(UiState::default());
    sync.tick(ts(100), changed(), &link);
    assert_eq!(sync.deadline(), Some(ts(100).saturating_add(DEBOUNCE)));
    assert_eq!(saves(&ends.drain()), 0);
    sync.tick(
        ts(100).saturating_add(Duration::from_millis(999)),
        changed(),
        &link,
    );
    assert_eq!(saves(&ends.drain()), 0);
    sync.tick(ts(101), changed(), &link);
    assert_eq!(saves(&ends.drain()), 1);
    sync.tick(ts(105), changed(), &link);
    assert_eq!(saves(&ends.drain()), 0, "saved states are not resent");
}

#[test]
fn flush_saves_immediately_and_only_when_different() {
    let (link, mut ends) = link(AppState::default());
    let mut sync = UiStateSync::new(UiState::default());
    sync.flush(UiState::default(), &link);
    assert_eq!(saves(&ends.drain()), 0);
    sync.flush(changed(), &link);
    assert_eq!(saves(&ends.drain()), 1);
}
