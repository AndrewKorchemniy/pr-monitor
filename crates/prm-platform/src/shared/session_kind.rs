//! Local versus remote desktop sessions (RDP), which pick the renderer.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionKind {
    Local,
    Rdp,
    ThinClient,
    OtherRemote,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    pub kind: SessionKind,
    /// The Windows session id.
    pub id: Option<u32>,
    /// A remote-display agent is installed (a hint; the session itself may still be local).
    pub remote_agent_host: bool,
}

impl SessionInfo {
    pub const fn is_remote(&self) -> bool {
        !matches!(self.kind, SessionKind::Local) || self.remote_agent_host
    }
}

/// `sm_remote` is `SM_REMOTESESSION`; `glass_mismatch` is the documented RemoteFX check
/// (`GlassSessionId` differs from ours). `SESSIONNAME` tells RDP sessions apart.
pub fn classify_session(
    sm_remote: bool,
    glass_mismatch: bool,
    session_name: Option<&str>,
) -> SessionKind {
    let name = session_name.unwrap_or_default().to_ascii_uppercase();
    if name.starts_with("TC-") {
        return SessionKind::ThinClient;
    }
    if !(sm_remote || glass_mismatch) {
        return SessionKind::Local;
    }
    if name.starts_with("RDP-") {
        SessionKind::Rdp
    } else {
        SessionKind::OtherRemote
    }
}

#[cfg(test)]
mod tests;
