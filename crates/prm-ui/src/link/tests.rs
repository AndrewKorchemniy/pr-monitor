use std::sync::Arc;

use prm_service::{AppEvent, AppState, Command};

use super::*;
use crate::testing::link;

#[test]
fn new_state_is_reported_once_then_read() {
    let (mut link, ends) = link(AppState::default());
    assert!(matches!(link.check(), StateCheck::Unchanged));
    let next = AppState {
        generation: 7,
        ..AppState::default()
    };
    ends.state.send_replace(Arc::new(next));
    assert!(matches!(link.check(), StateCheck::Changed));
    assert_eq!(link.latest().generation, 7);
    assert!(matches!(link.check(), StateCheck::Unchanged));
}

#[test]
fn a_gone_service_is_reported_once() {
    let (mut link, ends) = link(AppState::default());
    drop(ends.state);
    assert!(matches!(link.check(), StateCheck::Stopped));
    assert!(matches!(link.check(), StateCheck::Unchanged));
}

#[test]
fn commands_and_events_pass_through() {
    let (link, mut ends) = link(AppState::default());
    link.send(Command::RefreshNow);
    assert!(matches!(ends.drain().as_slice(), [Command::RefreshNow]));
    ends.events.send(AppEvent::Stopped).expect("receiver alive");
    assert_eq!(link.try_event(), Some(AppEvent::Stopped));
    assert_eq!(link.try_event(), None);
}
