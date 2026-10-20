//! The single-instance wire format: one newline-terminated JSON line each way, strictly
//! validated (size, encoding, schema, version, URI allow-list).

use prm_domain::uri::AppUri;
use serde::{Deserialize, Serialize};

pub const HANDOFF_VERSION: u8 = 1;
/// Longest frame, excluding the trailing newline.
pub const MAX_FRAME: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandoffRequest {
    /// Bring the window forward.
    Show,
    Activate(AppUri),
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandoffReply {
    Ok,
    Rejected(String),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    #[error("frame too long")]
    TooLong,
    #[error("missing newline")]
    NoNewline,
    #[error("not UTF-8")]
    NotUtf8,
    #[error("NUL byte")]
    Nul,
    #[error("bad json: {0}")]
    Json(String),
    #[error("unsupported version {0}")]
    Version(u64),
    #[error("uri not allowed for cmd")]
    UnexpectedUri,
    #[error("missing uri")]
    MissingUri,
    #[error("invalid uri: {0}")]
    Uri(String),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    v: u64,
    cmd: WireCommand,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    uri: Option<String>,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireCommand {
    Show,
    Activate,
    Quit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReply {
    v: u64,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn line<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

pub fn encode_request(request: &HandoffRequest) -> Vec<u8> {
    let (cmd, uri) = match request {
        HandoffRequest::Show => (WireCommand::Show, None),
        HandoffRequest::Activate(uri) => (WireCommand::Activate, Some(uri.to_string())),
        HandoffRequest::Quit => (WireCommand::Quit, None),
    };
    line(&WireRequest {
        v: u64::from(HANDOFF_VERSION),
        cmd,
        uri,
    })
}

pub fn encode_reply(reply: &HandoffReply) -> Vec<u8> {
    let (ok, error) = match reply {
        HandoffReply::Ok => (true, None),
        HandoffReply::Rejected(reason) => (false, Some(reason.clone())),
    };
    line(&WireReply {
        v: u64::from(HANDOFF_VERSION),
        ok,
        error,
    })
}

/// Size, newline, UTF-8 and NUL checks; returns the text without the newline.
fn frame_text(frame: &[u8]) -> Result<&str, CodecError> {
    if frame.len() > MAX_FRAME + 1 {
        return Err(CodecError::TooLong);
    }
    let body = frame.strip_suffix(b"\n").ok_or(CodecError::NoNewline)?;
    let text = std::str::from_utf8(body).map_err(|_| CodecError::NotUtf8)?;
    if text.contains('\0') {
        return Err(CodecError::Nul);
    }
    Ok(text)
}

fn check_version(v: u64) -> Result<(), CodecError> {
    if v == u64::from(HANDOFF_VERSION) {
        Ok(())
    } else {
        Err(CodecError::Version(v))
    }
}

/// `frame` includes the trailing newline.
pub fn decode_request(frame: &[u8]) -> Result<HandoffRequest, CodecError> {
    let text = frame_text(frame)?;
    let wire: WireRequest =
        serde_json::from_str(text).map_err(|e| CodecError::Json(e.to_string()))?;
    check_version(wire.v)?;
    match (wire.cmd, wire.uri) {
        (WireCommand::Show, None) => Ok(HandoffRequest::Show),
        (WireCommand::Quit, None) => Ok(HandoffRequest::Quit),
        (WireCommand::Show | WireCommand::Quit, Some(_)) => Err(CodecError::UnexpectedUri),
        (WireCommand::Activate, None) => Err(CodecError::MissingUri),
        (WireCommand::Activate, Some(uri)) => AppUri::parse(&uri)
            .map(HandoffRequest::Activate)
            .map_err(|e| CodecError::Uri(e.to_string())),
    }
}

pub fn decode_reply(frame: &[u8]) -> Result<HandoffReply, CodecError> {
    let text = frame_text(frame)?;
    let wire: WireReply =
        serde_json::from_str(text).map_err(|e| CodecError::Json(e.to_string()))?;
    check_version(wire.v)?;
    Ok(if wire.ok {
        HandoffReply::Ok
    } else {
        HandoffReply::Rejected(wire.error.unwrap_or_default())
    })
}

#[cfg(test)]
mod tests;
