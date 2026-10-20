//! Local versus remote desktop sessions, which pick the renderer.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionKind {
    Local,
    Rdp,
    OtherRemote,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    pub kind: SessionKind,
    /// The Windows session id.
    pub id: Option<u32>,
}

impl SessionInfo {
    pub const fn is_remote(&self) -> bool {
        !matches!(self.kind, SessionKind::Local)
    }
}

/// `sm_remote` is `SM_REMOTESESSION`; `glass_mismatch` is the documented RemoteFX check
/// (`GlassSessionId` differs from ours). `SESSIONNAME` tells RDP from other remote sessions.
pub fn classify_session(
    sm_remote: bool,
    glass_mismatch: bool,
    session_name: Option<&str>,
) -> SessionKind {
    if !(sm_remote || glass_mismatch) {
        return SessionKind::Local;
    }
    let name = session_name.unwrap_or_default().to_ascii_uppercase();
    if name.starts_with("RDP-") {
        SessionKind::Rdp
    } else {
        SessionKind::OtherRemote
    }
}

#[cfg(test)]
mod tests;
