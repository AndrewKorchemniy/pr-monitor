//! Files embedded at build time (generated or overridden in `build.rs`).

/// The 256 px app icon; also written to the data folder for toasts.
pub(crate) const ICON_256_PNG: &[u8] = include_bytes!(env!("PRM_ICON_256_PNG"));

/// Third-party license notices (plain text).
pub(crate) const THIRD_PARTY_LICENSES: &str =
    include_str!(concat!(env!("OUT_DIR"), "/THIRD-PARTY-LICENSES.txt"));
