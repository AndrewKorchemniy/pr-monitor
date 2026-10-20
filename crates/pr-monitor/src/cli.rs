//! Command-line arguments: a small hand-rolled parser (no dependencies).

use std::{ffi::OsString, path::PathBuf};

use prm_config::schema::RendererPref;
use prm_domain::app::URI_SCHEME;
use prm_ui::RendererKind;

/// More arguments than this is a mistake (or an attack through the protocol handler).
const MAX_ARGS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct Args {
    pub mode: Mode,
    pub minimized: bool,
    pub activate: Option<String>,
    pub debug: bool,
    pub renderer: Option<RendererPref>,
    pub home: Option<PathBuf>,
    /// Internal: this process is the renderer fallback started by `pid`.
    pub relaunched_after: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Mode {
    #[default]
    Run,
    SelfTest,
    Unregister {
        purge: bool,
    },
    Version,
    Help,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ArgError {
    #[error("unknown argument: {0}")]
    Unknown(String),
    #[error("{0} needs a value")]
    MissingValue(&'static str),
    #[error("invalid value for {flag}: {value}")]
    Invalid { flag: &'static str, value: String },
    #[error("{0} given more than once")]
    Duplicate(&'static str),
    #[error("{0} conflicts with {1}")]
    Conflict(&'static str, &'static str),
    #[error("argument is not valid Unicode")]
    NotUnicode,
    #[error("too many arguments")]
    TooMany,
}

/// The parser's working state: which mode flags were seen.
#[derive(Default)]
struct Seen {
    modes: Vec<&'static str>,
    purge: bool,
}

/// `args` excludes the program name. Accepts `--flag value` and `--flag=value`.
pub(crate) fn parse_args<I: IntoIterator<Item = OsString>>(args: I) -> Result<Args, ArgError> {
    let raw: Vec<OsString> = args.into_iter().take(MAX_ARGS + 1).collect();
    if raw.len() > MAX_ARGS {
        return Err(ArgError::TooMany);
    }
    let mut items = raw
        .into_iter()
        .map(|a| a.into_string().map_err(|_| ArgError::NotUnicode))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let mut args = Args::default();
    let mut seen = Seen::default();
    while let Some(item) = items.next() {
        let (flag, inline) = match item.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            Some(_) | None => (item.clone(), None),
        };
        let mut value = |name: &'static str| {
            inline
                .clone()
                .or_else(|| items.next())
                .ok_or(ArgError::MissingValue(name))
        };
        apply(&mut args, &mut seen, &flag, &mut value)?;
    }
    finish(args, &seen)
}

fn apply(
    args: &mut Args,
    seen: &mut Seen,
    flag: &str,
    value: &mut impl FnMut(&'static str) -> Result<String, ArgError>,
) -> Result<(), ArgError> {
    match flag {
        "--minimized" => set_once(&mut args.minimized, "--minimized")?,
        "--debug" => set_once(&mut args.debug, "--debug")?,
        "--purge" => seen.purge = true,
        "--self-test" | "--unregister" | "--version" | "--help" | "-h" | "-V" => {
            seen.modes.push(mode_name(flag));
        }
        "--activate" => put_once(&mut args.activate, value("--activate")?, "--activate")?,
        "--home" => put_once(&mut args.home, PathBuf::from(value("--home")?), "--home")?,
        "--renderer" => put_once(
            &mut args.renderer,
            renderer(&value("--renderer")?)?,
            "--renderer",
        )?,
        "--relaunched-after" => {
            let raw = value("--relaunched-after")?;
            let pid = raw.parse().map_err(|_| ArgError::Invalid {
                flag: "--relaunched-after",
                value: raw,
            })?;
            put_once(&mut args.relaunched_after, pid, "--relaunched-after")?;
        }
        other
            if other
                .to_ascii_lowercase()
                .starts_with(&format!("{URI_SCHEME}:")) =>
        {
            put_once(&mut args.activate, other.to_owned(), "--activate")?;
        }
        other => return Err(ArgError::Unknown(other.to_owned())),
    }
    Ok(())
}

fn mode_name(flag: &str) -> &'static str {
    match flag {
        "--self-test" => "--self-test",
        "--unregister" => "--unregister",
        "--version" | "-V" => "--version",
        _ => "--help",
    }
}

fn finish(mut args: Args, seen: &Seen) -> Result<Args, ArgError> {
    if let [first, second, ..] = seen.modes.as_slice() {
        return Err(if first == second {
            ArgError::Duplicate(first)
        } else {
            ArgError::Conflict(first, second)
        });
    }
    args.mode = match seen.modes.first().copied() {
        Some("--self-test") => Mode::SelfTest,
        Some("--unregister") => Mode::Unregister { purge: seen.purge },
        Some("--version") => Mode::Version,
        Some(_) => Mode::Help,
        None => Mode::Run,
    };
    if seen.purge && !matches!(args.mode, Mode::Unregister { .. }) {
        return Err(ArgError::Conflict("--purge", "anything but --unregister"));
    }
    Ok(args)
}

fn set_once(flag: &mut bool, name: &'static str) -> Result<(), ArgError> {
    if std::mem::replace(flag, true) {
        return Err(ArgError::Duplicate(name));
    }
    Ok(())
}

fn put_once<T>(slot: &mut Option<T>, value: T, name: &'static str) -> Result<(), ArgError> {
    if slot.replace(value).is_some() {
        return Err(ArgError::Duplicate(name));
    }
    Ok(())
}

fn renderer(value: &str) -> Result<RendererPref, ArgError> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(RendererPref::Auto),
        "glow" | "opengl" => Ok(RendererPref::Glow),
        "wgpu" => Ok(RendererPref::Wgpu),
        _ => Err(ArgError::Invalid {
            flag: "--renderer",
            value: value.to_owned(),
        }),
    }
}

pub(crate) const fn usage() -> &'static str {
    "Usage: pr-monitor [options]

  --minimized               Start hidden in the tray
  --activate <uri>          Open a prmonitor:// link (also accepted bare)
  --renderer <auto|glow|wgpu>
                            Choose the graphics backend
  --home <dir>              Keep data in <dir> instead of %LOCALAPPDATA%\\PRMonitor
  --debug                   Log to the console at debug level
  --self-test               Run built-in checks and exit
  --unregister [--purge]    Remove PR Monitor's system registrations (and data)
  --version                 Print the version
  --help                    Print this help"
}

/// The fallback relaunch keeps the user's flags, switches the renderer and names the parent.
pub(crate) fn relaunch_args(args: &Args, renderer: RendererKind, parent_pid: u32) -> Vec<OsString> {
    let mut out: Vec<OsString> = Vec::new();
    if args.minimized {
        out.push("--minimized".into());
    }
    if args.debug {
        out.push("--debug".into());
    }
    if let Some(home) = &args.home {
        out.push("--home".into());
        out.push(home.clone().into_os_string());
    }
    if let Some(uri) = &args.activate {
        out.push("--activate".into());
        out.push(uri.into());
    }
    out.push("--renderer".into());
    out.push(renderer.as_arg().into());
    out.push("--relaunched-after".into());
    out.push(parent_pid.to_string().into());
    out
}

#[cfg(test)]
mod tests;
