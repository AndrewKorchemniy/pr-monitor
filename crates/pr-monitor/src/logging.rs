//! Logging: daily files (`LOG_FILES_KEPT` kept, `MAX_DAY_BYTES` a day at most) in the data
//! folder, redacted; a console layer with `--debug`.
//! `PRMONITOR_LOG` overrides the level filter.

pub(crate) mod redact;
mod writer;

use std::{path::Path, str::FromStr};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    Registry, filter::Targets, fmt, layer::SubscriberExt as _, reload, util::SubscriberInitExt as _,
};

use self::writer::{DayBudget, MAX_DAY_BYTES, RedactingMakeWriter};

/// Daily log files kept; older ones are deleted on rotation.
const LOG_FILES_KEPT: usize = 8;

/// Third-party targets kept at warn or quieter.
const NOISY: [&str; 19] = [
    "wgpu_core",
    "wgpu_hal",
    "naga",
    "winit",
    "glutin",
    "egui_glow",
    "egui_wgpu",
    "eframe",
    "accesskit",
    "accesskit_windows",
    "fontique",
    "hyper",
    "hyper_util",
    "h2",
    "reqwest",
    "rustls",
    "zbus",
    "interprocess",
    "tracing",
];

pub(crate) struct LogHandle {
    reload: Option<reload::Handle<Targets, Registry>>,
    env_override: bool,
    /// The log file could be opened.
    pub file_ok: bool,
}

impl LogHandle {
    /// Applies the config's level once it is loaded (no-op when `PRMONITOR_LOG` is set).
    pub(crate) fn apply_level(&self, config_level: &str, debug: bool) {
        if self.env_override {
            return;
        }
        if let Some(handle) = &self.reload {
            let _ = handle.reload(log_targets(None, config_level, debug));
        }
    }
}

/// `env` verbatim when it parses; else debug or the config level, with noisy crates at warn.
pub(crate) fn log_targets(env: Option<&str>, config_level: &str, debug: bool) -> Targets {
    if let Some(targets) = env.and_then(|e| Targets::from_str(e).ok()) {
        return targets;
    }
    let level = if debug {
        LevelFilter::DEBUG
    } else {
        LevelFilter::from_str(config_level).unwrap_or(LevelFilter::INFO)
    };
    let noisy = level.min(LevelFilter::WARN);
    Targets::new()
        .with_default(level)
        .with_targets(NOISY.map(|t| (t, noisy)))
}

pub(crate) fn init(logs: &Path, debug: bool, env: Option<String>) -> LogHandle {
    let (filter, reload) = reload::Layer::new(log_targets(env.as_deref(), "info", debug));
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(prm_domain::app::APP_ID)
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(logs)
        .ok();
    let file_ok = appender.is_some();
    let file = appender.map(|a| {
        fmt::layer()
            .with_writer(RedactingMakeWriter::new(a).with_budget(DayBudget::daily(MAX_DAY_BYTES)))
            .with_ansi(false)
            .with_thread_names(true)
    });
    let console = debug.then(|| {
        fmt::layer()
            .with_writer(RedactingMakeWriter::new(std::io::stderr))
            .with_ansi(false)
    });
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(file)
        .with(console)
        .try_init()
        .is_ok();
    LogHandle {
        reload: installed.then_some(reload),
        env_override: env.is_some_and(|e| Targets::from_str(&e).is_ok()),
        file_ok,
    }
}

#[cfg(test)]
mod tests;
