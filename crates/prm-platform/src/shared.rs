//! Pure helpers compiled and tested on every OS: toast XML, the handoff wire format, instance names,
//! registry command strings, proxy lists, session classification and small formatting rules.
//! Only the wire format and the toast XML are public (for `--self-test`); the rest is re-exported
//! from the crate root where the bin needs it.

// why: these modules serve Windows adapters but compile everywhere so their tests run on CI's
// macOS and Linux too; there, their Windows-only items are unused.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod colors;
pub mod handoff_codec;
pub(crate) mod instance_key;
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod os_desc;
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod proxy_list;
pub(crate) mod registration;
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod run_command;
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod secret_names;
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Windows-only callers; tested on every OS")
)]
pub(crate) mod session_kind;
pub mod toast_xml;
