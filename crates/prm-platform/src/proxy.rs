//! The proxy decision for Azure DevOps (PAC/WPAD on Windows). Blocking.

use prm_domain::ports::PortError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProxyMode {
    /// Use the PAC/WPAD answer when the system has one; otherwise leave it to reqwest.
    System,
    /// Try auto-detect even when the system settings do not ask for it.
    PacOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyDecision {
    Direct,
    Proxy(url::Url),
    /// Static system settings: reqwest's `system-proxy` handles them.
    SystemStatic,
}

/// Call from a background thread only (WPAD can take seconds).
pub fn resolve_proxy(target: &url::Url, mode: ProxyMode) -> Result<ProxyDecision, PortError> {
    #[cfg(windows)]
    let decision = crate::win::winhttp::resolve_proxy(target, mode);
    #[cfg(not(windows))]
    let decision = {
        let _ = (target, mode);
        Ok(ProxyDecision::SystemStatic)
    };
    decision
}
