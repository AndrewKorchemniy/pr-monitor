//! Which renderer to start with, and remembering a failed one. OpenGL is the default (smallest,
//! fastest start); wgpu (DirectX 12, WARP without a GPU) over remote desktops, on arm64, and
//! after OpenGL failed on this version.

use std::path::PathBuf;

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RendererChoice {
    pub kind: RendererKind,
    pub reason: &'static str,
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

/// The reason when OpenGL failed on this version (logged at warn with the marker).
pub(crate) const GLOW_FAILED: &str = "previous OpenGL failure";

/// The first rule that applies wins.
pub(crate) fn choose_renderer(input: &RendererInputs<'_>) -> RendererChoice {
    let choice = |kind, reason| RendererChoice { kind, reason };
    if let Some(kind) = input.cli.and_then(explicit) {
        return choice(kind, "cli");
    }
    if let Some(kind) = explicit(input.config) {
        return choice(kind, "config");
    }
    if input.arch == "aarch64" {
        return choice(RendererKind::Wgpu, "arm64");
    }
    if input.remote {
        return choice(RendererKind::Wgpu, "remote session");
    }
    if input.marker.is_some_and(|m| glow_broke(m, input.version)) {
        return choice(RendererKind::Wgpu, GLOW_FAILED);
    }
    choice(RendererKind::Glow, "default")
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

/// Reads and writes the marker; failures are logged and otherwise ignored.
pub(crate) struct MarkerFile {
    path: PathBuf,
    version: &'static str,
}

impl MarkerFile {
    pub(crate) const fn new(path: PathBuf, version: &'static str) -> Self {
        Self { path, version }
    }

    pub(crate) fn load(&self) -> Option<GpuMarker> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Before the window opens: an attempt that never reaches `mark_ok` counts as a failure.
    pub(crate) fn begin(&self, kind: RendererKind) {
        let glow_failed = self.load().is_some_and(|m| glow_broke(&m, self.version));
        self.save(&GpuMarker {
            version: self.version.to_owned(),
            attempt: Some(kind),
            pid: std::process::id(),
            ok: false,
            glow_failed,
        });
    }

    /// The first frame was painted.
    pub(crate) fn mark_ok(&self) {
        if let Some(mut marker) = self.load() {
            marker.ok = true;
            self.save(&marker);
        }
    }

    pub(crate) fn mark_failed(&self, kind: RendererKind) {
        let mut marker = self.load().unwrap_or_default();
        self.version.clone_into(&mut marker.version);
        marker.ok = false;
        marker.glow_failed |= kind == RendererKind::Glow;
        self.save(&marker);
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
