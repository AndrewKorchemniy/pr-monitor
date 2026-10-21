//! A one-line OS description for About and diagnostics.

/// Windows 11 still reports "Windows 10" as its product name; build 22000 is the first 11.
const WINDOWS_11_BUILD: u32 = 22_000;

/// The registry facts the description is built from (any may be missing).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OsFacts<'a> {
    pub(crate) product: Option<&'a str>,
    pub(crate) display_version: Option<&'a str>,
    pub(crate) build: Option<u32>,
    pub(crate) ubr: Option<u32>,
    pub(crate) arch: &'a str,
}

/// For example `Windows 11 24H2 (26100.2033) x86_64`.
pub(crate) fn format_os_description(facts: &OsFacts<'_>) -> String {
    let mut product = facts.product.unwrap_or("Windows").to_owned();
    if facts.build.is_some_and(|b| b >= WINDOWS_11_BUILD) {
        product = product.replacen("Windows 10", "Windows 11", 1);
    }
    let mut parts = vec![product];
    if let Some(version) = facts.display_version.filter(|v| !v.is_empty()) {
        parts.push(version.to_owned());
    }
    match (facts.build, facts.ubr) {
        (Some(build), Some(ubr)) => parts.push(format!("({build}.{ubr})")),
        (Some(build), None) => parts.push(format!("({build})")),
        (None, _) => {}
    }
    if !facts.arch.is_empty() {
        parts.push(facts.arch.to_owned());
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests;
