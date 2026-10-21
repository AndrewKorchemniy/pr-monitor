use std::sync::{Mutex, OnceLock, PoisonError, Weak};

use super::*;

#[derive(Default)]
struct Recorder(Mutex<Vec<Command>>);

impl Recorder {
    fn taken(&self) -> Vec<String> {
        let commands = std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner));
        commands.iter().map(|c| format!("{c:?}")).collect()
    }
}

impl CommandSink for Recorder {
    fn send(&self, command: Command) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(command);
        true
    }
}

fn router() -> (ActivationRouter, Arc<AtomicBool>) {
    let quit = Arc::new(AtomicBool::new(false));
    (
        ActivationRouter::new(Arc::new(UiWaker::default()), Arc::clone(&quit)),
        quit,
    )
}

#[test]
fn activations_before_the_service_are_replayed_in_order() {
    let (router, _) = router();
    router.activate(HandoffRequest::Show);
    router.activate_raw("prmonitor://refresh");
    let recorder = Arc::new(Recorder::default());
    router.attach(Arc::clone(&recorder) as Arc<dyn CommandSink>);
    assert_eq!(
        recorder.taken(),
        ["Activate(Show { key: None })", "Activate(Refresh)"]
    );
    router.activate(HandoffRequest::Show);
    assert_eq!(
        recorder.taken().len(),
        1,
        "after attaching, activations pass straight through"
    );
}

/// A sink that, on its first command, starts another thread activating the router (as the
/// handoff listener would while the buffer is being replayed).
struct Interrupting {
    recorder: Recorder,
    router: OnceLock<Weak<ActivationRouter>>,
    started: AtomicBool,
    other: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl CommandSink for Interrupting {
    fn send(&self, command: Command) -> bool {
        if !self.started.swap(true, Ordering::Relaxed)
            && let Some(router) = self.router.get().and_then(Weak::upgrade)
        {
            let handle = std::thread::spawn(move || {
                router.activate(HandoffRequest::Activate(AppUri::Refresh));
            });
            *self.other.lock().unwrap_or_else(PoisonError::into_inner) = Some(handle);
        }
        self.recorder.send(command)
    }
}

/// Regression: an activation that arrived while the buffer was replayed reached the service
/// before older buffered ones, so the UI acted on a stale link last.
#[test]
fn activations_during_the_replay_wait_for_it() {
    let (router, _) = router();
    let router = Arc::new(router);
    router.activate(HandoffRequest::Show);
    router.activate_raw("prmonitor://show");
    let sink = Arc::new(Interrupting {
        recorder: Recorder::default(),
        router: OnceLock::new(),
        started: AtomicBool::new(false),
        other: Mutex::new(None),
    });
    let _ = sink.router.set(Arc::downgrade(&router));
    router.attach(Arc::clone(&sink) as Arc<dyn CommandSink>);
    let other = sink
        .other
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    other.expect("started").join().expect("joined");
    assert_eq!(
        sink.recorder.taken(),
        [
            "Activate(Show { key: None })",
            "Activate(Show { key: None })",
            "Activate(Refresh)"
        ]
    );
}

#[test]
fn the_buffer_keeps_only_the_newest() {
    let (router, _) = router();
    for _ in 0..(BUFFER + 4) {
        router.activate(HandoffRequest::Show);
    }
    router.activate_raw("prmonitor://refresh");
    let recorder = Arc::new(Recorder::default());
    router.attach(Arc::clone(&recorder) as Arc<dyn CommandSink>);
    let taken = recorder.taken();
    assert_eq!(taken.len(), BUFFER);
    assert_eq!(taken.last().map(String::as_str), Some("Activate(Refresh)"));
}

#[test]
fn quit_sets_the_flag_instead_of_sending() {
    let (router, quit) = router();
    let recorder = Arc::new(Recorder::default());
    router.attach(Arc::clone(&recorder) as Arc<dyn CommandSink>);
    router.activate(HandoffRequest::Quit);
    assert!(quit.load(Ordering::Relaxed));
    assert!(recorder.taken().is_empty(), "nothing is sent for quit");
}

#[test]
fn invalid_links_are_ignored() {
    let (router, _) = router();
    router.activate_raw("https://example.com");
    let recorder = Arc::new(Recorder::default());
    router.attach(Arc::clone(&recorder) as Arc<dyn CommandSink>);
    assert!(recorder.taken().is_empty(), "nothing was buffered");
}
