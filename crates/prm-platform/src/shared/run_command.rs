//! Command lines written to the registry (launch at login, the URL protocol) and the Task
//! Manager "Startup apps" approval bytes.

use std::path::{Path, PathBuf};

use prm_domain::{
    app::RUN_VALUE_NAME,
    ports::{PortError, PortErrorKind},
};

use crate::shared::instance_key::InstanceScope;

/// What Task Manager writes when a startup app is enabled.
pub const STARTUP_APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// `PRMonitor`, or `PRMonitor-<suffix>` for an isolated data root.
pub fn run_value_name(scope: InstanceScope, suffix: &str) -> String {
    match scope {
        InstanceScope::Default => RUN_VALUE_NAME.to_owned(),
        InstanceScope::Isolated => format!("{RUN_VALUE_NAME}-{suffix}"),
    }
}

/// A path is embedded in quotes, so it must not contain one.
fn quoted(path: &Path) -> Result<String, PortError> {
    let text = path.to_string_lossy();
    if text.contains('"') {
        return Err(PortError::new(
            PortErrorKind::Denied,
            "path contains a quote character",
        ));
    }
    Ok(format!("\"{text}\""))
}

/// `"exe" --minimized` [`--home "dir"`].
pub fn format_run_command(exe: &Path, home: Option<&Path>) -> Result<String, PortError> {
    let mut command = format!("{} --minimized", quoted(exe)?);
    if let Some(home) = home {
        command.push_str(" --home ");
        command.push_str(&quoted(home)?);
    }
    Ok(command)
}

/// The executable of a command line: the quoted first token, or the first word.
pub fn parse_run_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let exe = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    (!exe.is_empty()).then(|| PathBuf::from(exe))
}

/// `"exe" --activate "%1"` for the `prmonitor:` URL protocol.
pub fn protocol_command(exe: &Path) -> Result<String, PortError> {
    Ok(format!("{} --activate \"%1\"", quoted(exe)?))
}

/// Task Manager's approval flag: missing or empty means enabled; an even first byte means
/// enabled (0x02, 0x06), an odd one disabled (0x03, 0x07). Community-documented, not official.
pub fn startup_approved_enabled(bytes: Option<&[u8]>) -> bool {
    bytes
        .and_then(<[u8]>::first)
        .is_none_or(|first| first % 2 == 0)
}

#[cfg(test)]
mod tests;
