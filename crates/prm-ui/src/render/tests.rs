use std::sync::Arc;

use prm_config::WindowRect;

use super::*;

fn spec() -> LaunchSpec {
    LaunchSpec {
        renderer: RendererKind::Glow,
        start_hidden: false,
        hide_offscreen: false,
        window: None,
        icon: Arc::new(egui::IconData::default()),
    }
}

#[test]
fn renderer_kinds_flip_and_name_themselves() {
    assert_eq!(RendererKind::Glow.other(), RendererKind::Wgpu);
    assert_eq!(RendererKind::Wgpu.other(), RendererKind::Glow);
    assert_eq!(RendererKind::Wgpu.as_arg(), "wgpu");
    assert_eq!(RendererKind::Glow.to_eframe(), eframe::Renderer::Glow);
}

#[test]
fn software_adapters_are_recognized_by_name() {
    assert!(is_software_name("Microsoft Basic Render Driver"));
    assert!(is_software_name("llvmpipe (LLVM 17.0.6, 256 bits)"));
    assert!(!is_software_name("NVIDIA GeForce RTX 4070"));
}

#[test]
fn a_fresh_window_is_centered_at_the_default_size() {
    let options = native_options(&spec());
    assert!(options.centered);
    assert!(!options.persist_window);
    assert_eq!(options.viewport.inner_size, Some(egui::vec2(1280.0, 760.0)));
    assert_eq!(
        options.viewport.min_inner_size,
        Some(egui::vec2(360.0, 420.0))
    );
    assert_eq!(options.renderer, eframe::Renderer::Glow);
}

#[test]
fn a_saved_rect_is_restored() {
    let rect = WindowRect {
        x: 10.0,
        y: 20.0,
        w: 900.0,
        h: 600.0,
        maximized: true,
    };
    let restored = native_options(&LaunchSpec {
        window: Some(rect),
        ..spec()
    });
    assert!(!restored.centered);
    assert_eq!(restored.viewport.position, Some(egui::pos2(10.0, 20.0)));
    assert_eq!(restored.viewport.maximized, Some(true));
}

#[test]
fn start_hidden_without_cloaking_parks_the_window_off_screen() {
    let hidden = native_options(&LaunchSpec {
        start_hidden: true,
        hide_offscreen: true,
        ..spec()
    });
    assert_eq!(hidden.viewport.active, Some(false));
    assert_eq!(
        hidden.viewport.position,
        Some(egui::pos2(-32_000.0, -32_000.0))
    );
}
