//! The secondary instance's side: hand the request to the primary and exit. Synchronous, so a
//! secondary never builds a runtime.

use std::{
    io::{BufRead, BufReader, Read, Write},
    sync::mpsc,
    time::{Duration, Instant},
};

use interprocess::local_socket::{Stream, traits::Stream as _};

use crate::shared::{
    instance_key::InstanceKey,
    handoff_codec::{HandoffReply, HandoffRequest, MAX_FRAME, decode_reply, encode_request},
};

const CONNECT_RETRY: Duration = Duration::from_millis(100);
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForwardOutcome {
    Delivered,
    Rejected(String),
    /// Nobody is listening (the primary exited or never started).
    NoPrimary,
    /// Connected, but no valid reply in time.
    NotResponding,
}

/// Retries the connection until `connect_budget` runs out (the primary may still be starting).
pub fn forward_to_primary(
    key: &InstanceKey,
    request: &HandoffRequest,
    connect_budget: Duration,
) -> ForwardOutcome {
    let Some(stream) = connect(&key.pipe_name, connect_budget) else {
        return ForwardOutcome::NoPrimary;
    };
    if !crate::process::prepare_foreground_handoff(&stream) {
        return ForwardOutcome::NotResponding;
    }
    // why: Windows named pipes have no read/write timeouts, so the exchange runs on a helper
    // thread and we stop waiting after REPLY_TIMEOUT (the process exits right after anyway).
    let frame = encode_request(request);
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("handoff-client".into())
        .spawn(move || {
            let _ = tx.send(exchange(&stream, &frame));
        });
    if spawned.is_err() {
        return ForwardOutcome::NotResponding;
    }
    rx.recv_timeout(REPLY_TIMEOUT)
        .ok()
        .flatten()
        .unwrap_or(ForwardOutcome::NotResponding)
}

fn connect(pipe_name: &str, budget: Duration) -> Option<Stream> {
    let deadline = Instant::now() + budget;
    loop {
        let attempt = super::socket_name(pipe_name).and_then(Stream::connect);
        match attempt {
            Ok(stream) => return Some(stream),
            Err(_) if Instant::now() + CONNECT_RETRY < deadline => {
                std::thread::sleep(CONNECT_RETRY);
            }
            Err(_) => return None,
        }
    }
}

fn exchange(stream: &Stream, frame: &[u8]) -> Option<ForwardOutcome> {
    let mut writer = stream;
    writer.write_all(frame).ok()?;
    let mut reader = BufReader::new(stream);
    let limit = u64::try_from(MAX_FRAME + 1).unwrap_or(u64::MAX);
    let mut reply = Vec::with_capacity(64);
    reader
        .by_ref()
        .take(limit)
        .read_until(b'\n', &mut reply)
        .ok()?;
    Some(match decode_reply(&reply).ok()? {
        HandoffReply::Ok => ForwardOutcome::Delivered,
        HandoffReply::Rejected(reason) => ForwardOutcome::Rejected(reason),
    })
}
