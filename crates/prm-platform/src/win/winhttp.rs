//! The system's PAC/WPAD proxy decision, which reqwest's `system-proxy` cannot make.
//! Blocking (auto-detect can take seconds); run on a background thread only.

#![allow(
    unsafe_code,
    reason = "Win32 FFI; each block documents why it is sound"
)]

use std::ffi::c_void;

use prm_domain::ports::PortError;
use windows::{
    Win32::{
        Foundation::{GlobalFree, HGLOBAL},
        Networking::WinHttp::{
            ERROR_WINHTTP_LOGIN_FAILURE, WINHTTP_ACCESS_TYPE_NAMED_PROXY,
            WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_AUTO_DETECT_TYPE_DHCP,
            WINHTTP_AUTO_DETECT_TYPE_DNS_A, WINHTTP_AUTOPROXY_AUTO_DETECT,
            WINHTTP_AUTOPROXY_CONFIG_URL, WINHTTP_AUTOPROXY_OPTIONS,
            WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WINHTTP_PROXY_INFO, WinHttpCloseHandle,
            WinHttpGetIEProxyConfigForCurrentUser, WinHttpGetProxyForUrl, WinHttpOpen,
        },
    },
    core::{HRESULT, HSTRING, PCWSTR, PWSTR, w},
};

use super::port_error;
use crate::{
    proxy::{ProxyDecision, ProxyMode},
    shared::proxy_list::parse_proxy_list,
};

/// A string WinHTTP allocated with `GlobalAlloc`; freed on drop.
struct GlobalString(PWSTR);

impl GlobalString {
    fn text(&self) -> Option<String> {
        if self.0.is_null() {
            return None;
        }
        // SAFETY: WinHTTP returned a NUL-terminated string that lives until drop.
        unsafe { self.0.to_string() }.ok()
    }
}

impl Drop for GlobalString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: allocated by WinHTTP with GlobalAlloc; freed exactly once here.
            let _ = unsafe { GlobalFree(Some(HGLOBAL(self.0.0.cast()))) };
        }
    }
}

/// A WinHTTP session handle; closed on drop.
struct Session(*mut c_void);

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: a handle from WinHttpOpen, closed exactly once.
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

struct IeConfig {
    auto_detect: bool,
    pac_url: Option<String>,
}

fn ie_config() -> Result<IeConfig, PortError> {
    let mut ie = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    // SAFETY: `ie` is a local the call fills; its strings are freed by the guards below.
    unsafe { WinHttpGetIEProxyConfigForCurrentUser(&raw mut ie) }.map_err(|e| port_error(&e))?;
    let pac = GlobalString(ie.lpszAutoConfigUrl);
    let _proxy = GlobalString(ie.lpszProxy);
    let _bypass = GlobalString(ie.lpszProxyBypass);
    Ok(IeConfig {
        auto_detect: ie.fAutoDetect.as_bool(),
        pac_url: pac.text().filter(|u| !u.is_empty()),
    })
}

fn open_session() -> Result<Session, PortError> {
    // SAFETY: static NUL-terminated agent name; null proxy strings are allowed with NO_PROXY.
    let handle = unsafe {
        WinHttpOpen(
            w!("PRMonitor"),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    };
    if handle.is_null() {
        return Err(port_error(&windows::core::Error::from_thread()));
    }
    Ok(Session(handle))
}

/// What the system would use to reach `target`. `SystemStatic` leaves it to reqwest.
pub(crate) fn resolve_proxy(
    target: &url::Url,
    mode: ProxyMode,
) -> Result<ProxyDecision, PortError> {
    let ie = ie_config()?;
    if !ie.auto_detect && ie.pac_url.is_none() && mode == ProxyMode::System {
        return Ok(ProxyDecision::SystemStatic);
    }
    let auto = ie.auto_detect || ie.pac_url.is_none();
    let session = open_session()?;
    let pac = ie.pac_url.as_deref().map(HSTRING::from);
    let query = Query {
        session: &session,
        target,
        auto,
        pac: pac.as_ref(),
    };
    // Microsoft's pattern: try without credentials, then once with auto-logon.
    let login_failure = HRESULT::from_win32(ERROR_WINHTTP_LOGIN_FAILURE);
    match query.run(false) {
        Err(error) if error.code() == login_failure => query.run(true),
        other => other,
    }
    .map_err(|e| port_error(&e))
}

struct Query<'a> {
    session: &'a Session,
    target: &'a url::Url,
    auto: bool,
    pac: Option<&'a HSTRING>,
}

impl Query<'_> {
    fn run(&self, auto_logon: bool) -> windows::core::Result<ProxyDecision> {
        let mut flags = 0;
        if self.auto {
            flags |= WINHTTP_AUTOPROXY_AUTO_DETECT;
        }
        if self.pac.is_some() {
            flags |= WINHTTP_AUTOPROXY_CONFIG_URL;
        }
        let mut options = WINHTTP_AUTOPROXY_OPTIONS {
            dwFlags: flags,
            dwAutoDetectFlags: if self.auto {
                WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A
            } else {
                0
            },
            lpszAutoConfigUrl: self.pac.map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            fAutoLogonIfChallenged: auto_logon.into(),
            ..WINHTTP_AUTOPROXY_OPTIONS::default()
        };
        let mut info = WINHTTP_PROXY_INFO::default();
        let url = HSTRING::from(self.target.as_str());
        // SAFETY: the session is open; `url`, `options` and the PAC string outlive the call;
        // `info` receives GlobalAlloc strings wrapped in guards below.
        unsafe { WinHttpGetProxyForUrl(self.session.0, &url, &raw mut options, &raw mut info) }?;
        let proxy = GlobalString(info.lpszProxy);
        let _bypass = GlobalString(info.lpszProxyBypass);
        Ok(match info.dwAccessType {
            WINHTTP_ACCESS_TYPE_NO_PROXY => ProxyDecision::Direct,
            WINHTTP_ACCESS_TYPE_NAMED_PROXY => named_proxy(proxy.text()),
            _ => ProxyDecision::SystemStatic,
        })
    }
}

/// The system said "use a proxy": one we can parse, else leave it to the static settings
/// (going direct could fail on a network that blocks direct egress, e.g. a SOCKS-only PAC).
fn named_proxy(list: Option<String>) -> ProxyDecision {
    if let Some(url) = list.as_deref().and_then(parse_proxy_list) {
        return ProxyDecision::Proxy(url);
    }
    tracing::debug!(list = ?list, "unparsed proxy list; using the static settings");
    ProxyDecision::SystemStatic
}
