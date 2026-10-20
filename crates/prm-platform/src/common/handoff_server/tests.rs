use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use interprocess::local_socket::{Stream as SyncStream, traits::Stream as _};
use prm_domain::testing::key as pr_key;

use super::*;
use crate::common::handoff_client::{ForwardOutcome, forward_to_primary};

#[derive(Default)]
struct Recorder(Mutex<Vec<Activation>>);

impl ActivationSink for Recorder {
    fn activate(&self, activation: Activation) {
        self.0.lock().unwrap().push(activation);
    }
}

impl Recorder {
    fn seen(&self) -> Vec<Activation> {
        self.0.lock().unwrap().clone()
    }
}

/// A pipe name no other test (or run) uses.
fn unique_key() -> InstanceKey {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    InstanceKey {
        mutex_name: String::new(),
        pipe_name: format!("prm-test-{}-{n}", std::process::id()),
        lock_file: PathBuf::new(),
        suffix: String::new(),
    }
}

const BUDGET: Duration = Duration::from_secs(1);

#[test]
fn handoff_server_roundtrip_show() {
    let (key, sink) = (unique_key(), Arc::new(Recorder::default()));
    let server = HandoffServer::start(&key, sink.clone()).unwrap();

    let outcome = forward_to_primary(&key, &HandoffRequest::Show, BUDGET);

    assert_eq!(outcome, ForwardOutcome::Delivered);
    assert_eq!(sink.seen(), vec![Activation::Show]);
    assert!(server.stop(Duration::from_secs(2)));
}

#[test]
fn handoff_server_forwards_activation_uri() {
    let (key, sink) = (unique_key(), Arc::new(Recorder::default()));
    let server = HandoffServer::start(&key, sink.clone()).unwrap();
    let uri = AppUri::Show {
        key: Some(pr_key("contoso", 1, 22)),
    };

    let outcome = forward_to_primary(&key, &HandoffRequest::Activate(uri.clone()), BUDGET);

    assert_eq!(outcome, ForwardOutcome::Delivered);
    assert_eq!(sink.seen(), vec![Activation::Open(uri)]);
    assert!(server.stop(Duration::from_secs(2)));
}

#[test]
fn handoff_server_rejects_garbage_and_keeps_serving() {
    let (key, sink) = (unique_key(), Arc::new(Recorder::default()));
    let server = HandoffServer::start(&key, sink.clone()).unwrap();

    let stream = SyncStream::connect(super::super::socket_name(&key.pipe_name).unwrap()).unwrap();
    let mut reader = BufReader::new(stream);
    reader.get_mut().write_all(b"rm -rf /\n").unwrap();
    let mut reply = String::new();
    reader.read_line(&mut reply).unwrap();
    assert!(reply.contains("\"ok\":false"), "{reply}");

    assert_eq!(
        forward_to_primary(&key, &HandoffRequest::Show, BUDGET),
        ForwardOutcome::Delivered
    );
    assert_eq!(sink.seen(), vec![Activation::Show]);
    assert!(server.stop(Duration::from_secs(2)));
}

#[test]
fn handoff_client_reports_no_primary() {
    let outcome = forward_to_primary(
        &unique_key(),
        &HandoffRequest::Show,
        Duration::from_millis(250),
    );
    assert_eq!(outcome, ForwardOutcome::NoPrimary);
}

#[test]
fn handoff_server_stop_joins() {
    let key = unique_key();
    let server = HandoffServer::start(&key, Arc::new(Recorder::default())).unwrap();

    assert!(server.stop(Duration::from_secs(2)));

    let outcome = forward_to_primary(&key, &HandoffRequest::Show, Duration::from_millis(250));
    assert_eq!(outcome, ForwardOutcome::NoPrimary);
}
