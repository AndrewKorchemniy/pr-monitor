//! The primary instance's activation listener: its own thread and runtime, so it is up before
//! the service and the window exist. Connections are handled one at a time, each bounded.

use std::{
    io,
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use interprocess::local_socket::{ListenerOptions, tokio::Stream, traits::tokio::Listener as _};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::oneshot,
};

use crate::shared::{
    handoff_codec::{HandoffReply, HandoffRequest, READ_LIMIT, decode_request, encode_reply},
    instance_key::InstanceKey,
};

/// How long `start` waits for the listener to bind.
const READY_TIMEOUT: Duration = Duration::from_secs(2);
/// A client must send its frame within this long.
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_POLL: Duration = Duration::from_millis(10);
/// The first pause after a failed accept; doubled per consecutive failure.
const ACCEPT_BACKOFF_MIN: Duration = Duration::from_millis(100);
/// The longest pause between accept attempts.
const ACCEPT_BACKOFF_MAX: Duration = Duration::from_secs(2);

/// Receives activations on the handoff thread; must not block.
pub trait ActivationSink: Send + Sync + 'static {
    /// One validated request, in arrival order.
    fn activate(&self, request: HandoffRequest);
}

/// The running listener. Dropping it (or `stop`) ends the serve loop: the stop sender's drop
/// wakes it.
pub struct HandoffServer {
    stop: oneshot::Sender<()>,
    thread: JoinHandle<()>,
}

impl HandoffServer {
    /// Binds before returning (or fails), then serves on the `handoff` thread.
    pub fn start(key: &InstanceKey, sink: Arc<dyn ActivationSink>) -> io::Result<Self> {
        let name = key.pipe_name.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<io::Result<()>>();
        let (stop_tx, stop_rx) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("handoff".into())
            .spawn(move || {
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(serve(&name, sink, &ready_tx, stop_rx)),
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })?;
        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                stop: stop_tx,
                thread,
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "activation listener did not start",
            )),
        }
    }

    /// Stops accepting and waits up to `timeout`. `true` if the thread finished.
    pub fn stop(self, timeout: Duration) -> bool {
        let Self { stop, thread } = self;
        let _ = stop.send(());
        let deadline = Instant::now() + timeout;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(STOP_POLL);
        }
        thread.is_finished() && thread.join().is_ok()
    }
}

/// Pacing after failed accepts, so a persistent error (no pipe instance, no file descriptors)
/// neither spins a core nor floods the log: one warning per run of failures.
#[derive(Debug)]
struct AcceptBackoff {
    failures: u32,
    delay: Duration,
}

impl Default for AcceptBackoff {
    fn default() -> Self {
        Self {
            failures: 0,
            delay: ACCEPT_BACKOFF_MIN,
        }
    }
}

impl AcceptBackoff {
    /// Records a failure; returns how long to wait before the next accept.
    fn failed(&mut self, error: &io::Error) -> Duration {
        if self.failures == 0 {
            tracing::warn!(%error, "activation accept failed; backing off");
        }
        self.failures = self.failures.saturating_add(1);
        let delay = self.delay;
        self.delay = (self.delay * 2).min(ACCEPT_BACKOFF_MAX);
        delay
    }

    /// Records a success, ending a run of failures.
    fn succeeded(&mut self) {
        if self.failures > 0 {
            tracing::info!(failures = self.failures, "activation accept recovered");
        }
        *self = Self::default();
    }
}

async fn serve(
    name: &str,
    sink: Arc<dyn ActivationSink>,
    ready: &mpsc::Sender<io::Result<()>>,
    mut stop: oneshot::Receiver<()>,
) {
    let listener = match super::socket_name(name).and_then(|n| {
        ListenerOptions::new()
            .name(n)
            .try_overwrite(cfg!(target_os = "macos"))
            .create_tokio()
    }) {
        Ok(listener) => {
            let _ = ready.send(Ok(()));
            listener
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut backoff = AcceptBackoff::default();
    loop {
        let delay = tokio::select! {
            _ = &mut stop => break,
            accepted = listener.accept() => match accepted {
                Ok(conn) => {
                    backoff.succeeded();
                    handle(conn, sink.as_ref()).await;
                    continue;
                }
                Err(error) => backoff.failed(&error),
            },
        };
        tokio::select! {
            _ = &mut stop => break,
            () = tokio::time::sleep(delay) => {}
        }
    }
}

/// One request per connection; malformed frames get a rejection, never a panic.
async fn handle(conn: Stream, sink: &dyn ActivationSink) {
    if !crate::process::peer_in_our_session(&conn) {
        tracing::warn!("activation from another session ignored");
        return;
    }
    let mut reader = BufReader::new(&conn).take(READ_LIMIT);
    let mut frame = Vec::with_capacity(128);
    let read = tokio::time::timeout(READ_TIMEOUT, reader.read_until(b'\n', &mut frame)).await;
    if !matches!(read, Ok(Ok(_))) {
        return;
    }
    let reply = match decode_request(&frame) {
        Ok(request) => {
            sink.activate(request);
            HandoffReply::Ok
        }
        Err(error) => {
            tracing::warn!(%error, "activation rejected");
            HandoffReply::Rejected(error.to_string())
        }
    };
    let mut writer = &conn;
    let _ = tokio::time::timeout(WRITE_TIMEOUT, writer.write_all(&encode_reply(&reply))).await;
}

#[cfg(test)]
mod tests;
