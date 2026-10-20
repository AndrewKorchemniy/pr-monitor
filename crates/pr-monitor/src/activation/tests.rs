use std::sync::{Mutex, PoisonError};

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
    router.activate(Activation::Show);
    router.activate_raw("prmonitor://refresh");
    let recorder = Arc::new(Recorder::default());
    router.attach(Arc::clone(&recorder) as Arc<dyn CommandSink>);
    assert_eq!(
        recorder.taken(),
        ["Activate(Show { key: None })", "Activate(Refresh)"]
    );
    router.activate(Activation::Show);
    assert_eq!(
        recorder.taken().len(),
        1,
        "after attaching, activations pass straight through"
    );
}

#[test]
fn the_buffer_keeps_only_the_newest() {
    let (router, _) = router();
    for _ in 0..(BUFFER + 4) {
        router.activate(Activation::Show);
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
    router.activate(Activation::Quit);
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
