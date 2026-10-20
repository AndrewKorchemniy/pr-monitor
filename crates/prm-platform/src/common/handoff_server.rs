//! The primary instance's activation listener: its own thread and runtime, so it is up before
//! the service and the window exist. Connections are handled one at a time, each bounded.

use std::{
    io,
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use interprocess::local_socket::{ListenerOptions, tokio::Stream, traits::tokio::Listener as _};
use prm_domain::uri::AppUri;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::oneshot,
};

use crate::shared::{
    handoff_codec::{HandoffReply, HandoffRequest, MAX_FRAME, decode_request, encode_reply},
    instance_key::InstanceKey,
};

/// How long `start` waits for the listener to bind.
const READY_TIMEOUT: Duration = Duration::from_secs(2);
/// A client must send its frame within this long.
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation {
    Show,
    Open(AppUri),
    Quit,
}

/// Receives activations on the handoff thread; must not block.
pub trait ActivationSink: Send + Sync + 'static {
    fn activate(&self, activation: Activation);
}

pub struct HandoffServer {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
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
                stop: Some(stop_tx),
                thread: Some(thread),
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
    pub fn stop(mut self, timeout: Duration) -> bool {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let Some(thread) = self.thread.take() else {
            return true;
        };
        let deadline = Instant::now() + timeout;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(STOP_POLL);
        }
        thread.is_finished() && thread.join().is_ok()
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
    loop {
        tokio::select! {
            _ = &mut stop => break,
            accepted = listener.accept() => match accepted {
                Ok(conn) => handle(conn, sink.as_ref()).await,
                Err(error) => tracing::warn!(%error, "activation accept failed"),
            },
        }
    }
}

fn activation(request: HandoffRequest) -> Activation {
    match request {
        HandoffRequest::Show => Activation::Show,
        HandoffRequest::Activate(uri) => Activation::Open(uri),
        HandoffRequest::Quit => Activation::Quit,
    }
}

/// One request per connection; malformed frames get a rejection, never a panic.
async fn handle(conn: Stream, sink: &dyn ActivationSink) {
    if !crate::process::peer_in_our_session(&conn) {
        tracing::warn!("activation from another session ignored");
        return;
    }
    let limit = u64::try_from(MAX_FRAME + 1).unwrap_or(u64::MAX);
    let mut reader = BufReader::new(&conn).take(limit);
    let mut frame = Vec::with_capacity(128);
    let read = tokio::time::timeout(READ_TIMEOUT, reader.read_until(b'\n', &mut frame)).await;
    if !matches!(read, Ok(Ok(_))) {
        return;
    }
    let reply = match decode_request(&frame) {
        Ok(request) => {
            sink.activate(activation(request));
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
