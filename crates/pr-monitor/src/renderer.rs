//! Which renderer to start with, and remembering a failed one. OpenGL is the default (smallest,
//! fastest start); wgpu (DirectX 12, WARP without a GPU) over remote desktops, on arm64, and
//! after OpenGL failed on this version.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    thread::JoinHandle,
};

use prm_config::schema::RendererPref;
use prm_ui::RendererKind;
use serde::{Deserialize, Serialize};

/// `cache/renderer.json`: the last attempt and whether it reached the first frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct GpuMarker {
    pub version: String,
    pub attempt: Option<RendererKind>,
    pub pid: u32,
    pub ok: bool,
    pub glow_failed: bool,
}

pub(crate) struct RendererInputs<'a> {
    pub cli: Option<RendererPref>,
    pub config: RendererPref,
    pub remote: bool,
    pub arch: &'a str,
    pub marker: Option<&'a GpuMarker>,
    pub version: &'a str,
}

/// Why `choose_renderer` picked what it did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RendererReason {
    Cli,
    Config,
    Arm64,
    RemoteSession,
    /// OpenGL failed on this version (logged at warn with the marker).
    GlowFailed,
    Default,
}

impl RendererReason {
    /// For the log.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Config => "config",
            Self::Arm64 => "arm64",
            Self::RemoteSession => "remote session",
            Self::GlowFailed => "previous OpenGL failure",
            Self::Default => "default",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RendererChoice {
    pub kind: RendererKind,
    pub reason: RendererReason,
}

const fn explicit(pref: RendererPref) -> Option<RendererKind> {
    match pref {
        RendererPref::Glow => Some(RendererKind::Glow),
        RendererPref::Wgpu => Some(RendererKind::Wgpu),
        RendererPref::Auto => None,
    }
}

/// OpenGL failed on this version: recorded as failed, or attempted without reaching a frame.
/// Both `choose_renderer` and `MarkerFile::begin` use it, so a crashed OpenGL start is not
/// forgotten after one good wgpu run.
pub(crate) fn glow_broke(m: &GpuMarker, version: &str) -> bool {
    m.version == version && (m.glow_failed || (m.attempt == Some(RendererKind::Glow) && !m.ok))
}

/// The first rule that applies wins.
pub(crate) fn choose_renderer(input: &RendererInputs<'_>) -> RendererChoice {
    let choice = |kind, reason| RendererChoice { kind, reason };
    if let Some(kind) = input.cli.and_then(explicit) {
        return choice(kind, RendererReason::Cli);
    }
    if let Some(kind) = explicit(input.config) {
        return choice(kind, RendererReason::Config);
    }
    if input.arch == "aarch64" {
        return choice(RendererKind::Wgpu, RendererReason::Arm64);
    }
    if input.remote {
        return choice(RendererKind::Wgpu, RendererReason::RemoteSession);
    }
    if input.marker.is_some_and(|m| glow_broke(m, input.version)) {
        return choice(RendererKind::Wgpu, RendererReason::GlowFailed);
    }
    choice(RendererKind::Glow, RendererReason::Default)
}

/// Errors that mean "this renderer cannot start here" (worth one relaunch with the other).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "eframe::Error is foreign; every other error is fatal"
)]
pub(crate) fn is_renderer_init_error(error: &eframe::Error, tried: RendererKind) -> bool {
    match error {
        eframe::Error::Glutin(_)
        | eframe::Error::NoGlutinConfigs(..)
        | eframe::Error::OpenGL(_) => tried == RendererKind::Glow,
        eframe::Error::Wgpu(_) => tried == RendererKind::Wgpu,
        _ => false,
    }
}

/// Reads and writes the marker; failures are logged and otherwise ignored. It remembers what
/// `begin` wrote, so later updates never read the file back.
pub(crate) struct MarkerFile {
    path: PathBuf,
    version: &'static str,
    written: Mutex<Option<GpuMarker>>,
}

impl MarkerFile {
    pub(crate) const fn new(path: PathBuf, version: &'static str) -> Self {
        Self {
            path,
            version,
            written: Mutex::new(None),
        }
    }

    pub(crate) fn load(&self) -> Option<GpuMarker> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Before the window opens (`previous` is what `load` returned): an attempt that never
    /// reaches `mark_ok` counts as a failure.
    pub(crate) fn begin(&self, previous: Option<&GpuMarker>, kind: RendererKind) {
        let marker = GpuMarker {
            version: self.version.to_owned(),
            attempt: Some(kind),
            pid: std::process::id(),
            ok: false,
            glow_failed: previous.is_some_and(|m| glow_broke(m, self.version)),
        };
        self.save(&marker);
        *self.lock() = Some(marker);
    }

    /// The first frame was painted. Called on the UI thread before that frame is presented, so
    /// the write (fsync, retries) happens on a short-lived thread instead; the caller may detach
    /// the returned handle.
    pub(crate) fn mark_ok(self: &Arc<Self>) -> Option<JoinHandle<()>> {
        let marker = Arc::clone(self);
        std::thread::Builder::new()
            .name("marker".into())
            .spawn(move || marker.record_ok())
            .inspect_err(|error| tracing::warn!(%error, "couldn't start the renderer marker write"))
            .ok()
    }

    /// What `mark_ok` writes: the attempt worked, and an OpenGL run that worked clears an
    /// earlier OpenGL failure.
    pub(crate) fn record_ok(&self) {
        let marker = {
            let mut written = self.lock();
            let Some(marker) = written.as_mut() else {
                return;
            };
            marker.ok = true;
            if marker.attempt == Some(RendererKind::Glow) {
                marker.glow_failed = false;
            }
            marker.clone()
        };
        self.save(&marker);
    }

    /// Only before the first frame, so it never races `mark_ok`.
    pub(crate) fn mark_failed(&self, kind: RendererKind) {
        let marker = {
            let mut written = self.lock();
            let marker = written.get_or_insert_with(GpuMarker::default);
            self.version.clone_into(&mut marker.version);
            marker.ok = false;
            marker.glow_failed |= kind == RendererKind::Glow;
            marker.clone()
        };
        self.save(&marker);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<GpuMarker>> {
        self.written.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self, marker: &GpuMarker) {
        let written = serde_json::to_vec(marker)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                prm_config::atomic_write(&self.path, &bytes).map_err(|e| e.to_string())
            });
        if let Err(error) = written {
            tracing::warn!(%error, "couldn't write the renderer marker");
        }
    }
}

#[cfg(test)]
mod tests;
