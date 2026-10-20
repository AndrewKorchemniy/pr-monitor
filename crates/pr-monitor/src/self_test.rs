//! `--self-test`: headless checks of what the exe embeds. No lock, no files, no registry, no
//! network, no window, so it runs even next to a running instance. Exit 0 or 3.

use std::{ffi::OsString, io::Write as _, process::ExitCode};

use prm_config::{Config, normalize};
use prm_domain::{
    OrgId, PrKey,
    ids::{PrNumber, RepoId},
    uri::AppUri,
};
use prm_platform::shared::{
    handoff_codec::{HandoffRequest, decode_request, encode_request},
    toast_xml::{spec_from, toast_xml},
};

use crate::{build_info, cli, logging::redact::redact};

/// A link to one PR, as a toast button would carry.
fn sample_uri() -> AppUri {
    let key = OrgId::parse("contoso")
        .ok()
        .map(|org| PrKey::new(org, RepoId::from_u128(7), PrNumber(4711)));
    AppUri::Show { key }
}

pub(crate) struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn check(name: &'static str, result: Result<(), String>) -> Check {
    match result {
        Ok(()) => Check {
            name,
            ok: true,
            detail: String::new(),
        },
        Err(detail) => Check {
            name,
            ok: false,
            detail,
        },
    }
}

fn config_normalizes() -> Result<(), String> {
    let (_, issues) = normalize(Config::default());
    match issues
        .iter()
        .find(|i| i.severity == prm_config::Severity::Error)
    {
        Some(issue) => Err(format!("{issue:?}")),
        None => Ok(()),
    }
}

fn config_round_trips() -> Result<(), String> {
    let config = Config::default();
    let text = toml::to_string(&config).map_err(|e| e.to_string())?;
    let back: Config = toml::from_str(&text).map_err(|e| e.to_string())?;
    (back == config)
        .then_some(())
        .ok_or_else(|| "differs after a round trip".to_owned())
}

fn themes() -> Result<(), String> {
    let count = prm_theme::self_check()?;
    (count >= 1)
        .then_some(())
        .ok_or_else(|| "no built-in themes".to_owned())
}

fn icon() -> Result<(), String> {
    let png = crate::assets::ICON_256_PNG;
    (png.starts_with(b"\x89PNG") && png.len() > 1_000)
        .then_some(())
        .ok_or_else(|| "the icon is not a PNG".to_owned())
}

fn toast() -> Result<(), String> {
    let xml = toast_xml(&spec_from(&prm_domain::notify::Notification::test()));
    prm_platform::notify::validate_toast_xml(&xml).map_err(|e| e.to_string())
}

fn handoff_codec() -> Result<(), String> {
    let request = HandoffRequest::Activate(sample_uri());
    let back = decode_request(&encode_request(&request)).map_err(|e| format!("{e:?}"))?;
    (back == request)
        .then_some(())
        .ok_or_else(|| "differs after a round trip".to_owned())
}

fn uri() -> Result<(), String> {
    let uri = sample_uri();
    let back = AppUri::parse(&uri.to_string()).map_err(|e| e.to_string())?;
    (back == uri)
        .then_some(())
        .ok_or_else(|| "differs after a round trip".to_owned())
}

fn relaunch_args() -> Result<(), String> {
    let args = cli::Args {
        minimized: true,
        ..cli::Args::default()
    };
    let argv: Vec<OsString> = cli::relaunch_args(&args, prm_ui::RendererKind::Wgpu, 42);
    cli::parse_args(argv).map(|_| ()).map_err(|e| e.to_string())
}

fn redaction() -> Result<(), String> {
    let secret = "a".repeat(52);
    let line = format!(
        "Authorization: Bearer abcdefghijkl token={secret} eyJhbGciOi.eyJzdWIiOi.c2lnbmF0dXJl"
    );
    let clean = redact(&line);
    let leaked = ["abcdefghijkl", secret.as_str(), "eyJzdWIiOi"]
        .iter()
        .any(|s| clean.contains(s));
    (!leaked)
        .then_some(())
        .ok_or_else(|| format!("leaked: {clean}"))
}

pub(crate) fn checks() -> Vec<Check> {
    vec![
        check("config.default.normalize", config_normalizes()),
        check("config.default.roundtrip", config_round_trips()),
        check("themes.builtin", themes()),
        check("assets.icon", icon()),
        check("toast.xml", toast()),
        check("handoff.codec", handoff_codec()),
        check("uri.roundtrip", uri()),
        check("cli.relaunch", relaunch_args()),
        check("logging.redaction", redaction()),
    ]
}

/// One line per check, then a JSON summary line.
pub(crate) fn run() -> ExitCode {
    let results = checks();
    let mut out = prm_platform::console::console_out();
    for c in &results {
        let line = if c.ok {
            format!("ok   {}", c.name)
        } else {
            format!("FAIL {}: {}", c.name, c.detail)
        };
        let _ = writeln!(out, "{line}");
    }
    let ok = results.iter().all(|c| c.ok);
    let summary = serde_json::json!({
        "self_test": if ok { "ok" } else { "fail" },
        "version": build_info::VERSION,
        "build_sha": build_info::GIT_SHA,
        "checks": results.len(),
    });
    let _ = writeln!(out, "{summary}");
    let _ = out.flush();
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(crate::startup::EXIT_SELF_TEST)
    }
}

#[cfg(test)]
mod tests;
