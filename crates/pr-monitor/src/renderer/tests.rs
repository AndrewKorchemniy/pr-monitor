use super::*;

const VERSION: &str = "1.2.3";

fn inputs(marker: Option<&GpuMarker>) -> RendererInputs<'_> {
    RendererInputs {
        cli: None,
        config: RendererPref::Auto,
        remote: false,
        arch: "x86_64",
        marker,
        version: VERSION,
    }
}

fn marker(attempt: RendererKind, ok: bool, glow_failed: bool, version: &str) -> GpuMarker {
    GpuMarker {
        version: version.to_owned(),
        attempt: Some(attempt),
        pid: 1,
        ok,
        glow_failed,
    }
}

#[test]
fn opengl_is_the_default_on_a_local_x64_session() {
    assert_eq!(
        choose_renderer(&inputs(None)),
        RendererChoice {
            kind: RendererKind::Glow,
            reason: "default"
        }
    );
}

#[test]
fn explicit_choices_win_cli_first() {
    let both = RendererInputs {
        cli: Some(RendererPref::Glow),
        config: RendererPref::Wgpu,
        remote: true,
        ..inputs(None)
    };
    assert_eq!(choose_renderer(&both).reason, "cli");
    let config = RendererInputs {
        config: RendererPref::Wgpu,
        ..inputs(None)
    };
    assert_eq!(
        choose_renderer(&config),
        RendererChoice {
            kind: RendererKind::Wgpu,
            reason: "config"
        }
    );
}

#[test]
fn arm64_and_remote_sessions_use_wgpu() {
    let arm = RendererInputs {
        arch: "aarch64",
        ..inputs(None)
    };
    assert_eq!(choose_renderer(&arm).reason, "arm64");
    let remote = RendererInputs {
        remote: true,
        ..inputs(None)
    };
    assert_eq!(
        choose_renderer(&remote),
        RendererChoice {
            kind: RendererKind::Wgpu,
            reason: "remote session"
        }
    );
}

#[test]
fn a_failed_or_crashed_opengl_start_on_this_version_switches_to_wgpu() {
    for broken in [
        marker(RendererKind::Glow, false, false, VERSION),
        marker(RendererKind::Wgpu, true, true, VERSION),
    ] {
        assert_eq!(
            choose_renderer(&inputs(Some(&broken))).kind,
            RendererKind::Wgpu,
            "{broken:?}"
        );
    }
    let fine = marker(RendererKind::Glow, true, false, VERSION);
    assert_eq!(
        choose_renderer(&inputs(Some(&fine))).kind,
        RendererKind::Glow
    );
    let old = marker(RendererKind::Glow, false, true, "0.9.0");
    assert_eq!(
        choose_renderer(&inputs(Some(&old))).kind,
        RendererKind::Glow,
        "a new version tries OpenGL again"
    );
}

#[test]
fn only_backend_errors_trigger_a_relaunch() {
    let gl = eframe::Error::OpenGL(eframe::egui_glow::PainterError::from(
        "no context".to_owned(),
    ));
    assert!(is_renderer_init_error(&gl, RendererKind::Glow));
    assert!(!is_renderer_init_error(&gl, RendererKind::Wgpu));
    let creation = eframe::Error::AppCreation("bad".into());
    assert!(!is_renderer_init_error(&creation, RendererKind::Glow));
}

#[test]
fn the_marker_records_attempts_success_and_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = MarkerFile::new(dir.path().join("renderer.json"), VERSION);
    assert_eq!(file.load(), None);
    file.begin(RendererKind::Glow);
    let started = file.load().expect("written");
    assert_eq!(
        (started.attempt, started.ok),
        (Some(RendererKind::Glow), false)
    );
    file.mark_ok();
    assert!(file.load().expect("written").ok);
    file.mark_failed(RendererKind::Glow);
    file.begin(RendererKind::Wgpu);
    let after = file.load().expect("written");
    assert!(after.glow_failed, "a failure is remembered across attempts");
    assert_eq!(after.attempt, Some(RendererKind::Wgpu));
}

/// Regression: an OpenGL start that crashed before its first frame was forgotten after one good
/// wgpu run, so a machine with a broken OpenGL driver crashed on every second launch.
#[test]
fn renderer_marker_remembers_a_crashed_opengl_start_after_a_good_wgpu_run() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = MarkerFile::new(dir.path().join("renderer.json"), VERSION);
    file.begin(RendererKind::Glow);
    let after_crash = file.load().expect("written");
    assert_eq!(
        choose_renderer(&inputs(Some(&after_crash))).kind,
        RendererKind::Wgpu,
        "{after_crash:?}"
    );
    file.begin(RendererKind::Wgpu);
    file.mark_ok();
    let after_wgpu = file.load().expect("written");
    assert_eq!(
        choose_renderer(&inputs(Some(&after_wgpu))),
        RendererChoice {
            kind: RendererKind::Wgpu,
            reason: "previous OpenGL failure"
        },
        "{after_wgpu:?}"
    );
}
