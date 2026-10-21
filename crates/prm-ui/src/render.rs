//! Renderer choice and window options. The bin decides glow vs wgpu (with its relaunch
//! fallback); this module turns the decision into `NativeOptions`.

pub(crate) mod hover_bands;
pub(crate) mod policy;
pub(crate) mod repaint;

use std::sync::Arc;

use eframe::{
    egui,
    egui_wgpu::{self, WgpuConfiguration, WgpuSetup, wgpu},
};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    /// OpenGL: smallest memory and fastest start.
    Glow,
    /// DirectX 12 (WARP without a GPU), Metal or Vulkan.
    Wgpu,
}

impl RendererKind {
    pub const fn other(self) -> Self {
        match self {
            Self::Glow => Self::Wgpu,
            Self::Wgpu => Self::Glow,
        }
    }

    pub const fn to_eframe(self) -> eframe::Renderer {
        match self {
            Self::Glow => eframe::Renderer::Glow,
            Self::Wgpu => eframe::Renderer::Wgpu,
        }
    }

    /// The `--renderer` value.
    pub const fn as_arg(self) -> &'static str {
        match self {
            Self::Glow => "glow",
            Self::Wgpu => "wgpu",
        }
    }

    pub const fn api(self) -> &'static str {
        match self {
            Self::Glow => "OpenGL",
            Self::Wgpu => "DirectX 12",
        }
    }
}

/// What the UI needs to know about the chosen renderer and session.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RenderEnv {
    pub renderer: RendererKind,
    pub remote_session: bool,
    /// Set when this run is the fallback after the other renderer failed.
    pub fallback_from: Option<RendererKind>,
}

/// Everything `native_options` needs.
pub struct LaunchSpec {
    pub renderer: RendererKind,
    pub start_hidden: bool,
    /// Off-screen position as the start-hidden fallback (non-Windows; Windows cloaks).
    pub hide_offscreen: bool,
    /// The saved window rect, only when it is worth restoring (plausible and on a monitor).
    pub window: Option<prm_config::WindowRect>,
    pub icon: Arc<egui::IconData>,
}

const DEFAULT_SIZE: [f32; 2] = [1280.0, 760.0];
/// The smallest window, in points; the screens must stay usable at this size.
pub(crate) const MIN_SIZE: [f32; 2] = [360.0, 420.0];

pub fn native_options(spec: &LaunchSpec) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(prm_domain::app::APP_DISPLAY_NAME)
        .with_app_id(prm_domain::app::APP_ID)
        .with_icon(spec.icon.clone())
        .with_inner_size(DEFAULT_SIZE)
        .with_min_inner_size(MIN_SIZE)
        .with_drag_and_drop(false);
    let mut centered = true;
    if let Some(rect) = spec.window {
        viewport = viewport
            .with_position([rect.x, rect.y])
            .with_inner_size([rect.w, rect.h])
            .with_maximized(rect.maximized);
        centered = false;
    }
    if spec.start_hidden {
        viewport = viewport.with_active(false);
        if spec.hide_offscreen {
            viewport = viewport.with_position(crate::window::OFFSCREEN);
            centered = false;
        }
    }
    let mut options = eframe::NativeOptions {
        viewport,
        renderer: spec.renderer.to_eframe(),
        wgpu_options: wgpu_configuration(),
        centered,
        persist_window: false,
        system_font_fallback: true,
        dithering: false,
        run_and_return: true,
        clipboard_shortcuts: true,
        ..Default::default()
    };
    options.glow_options.hardware_acceleration = eframe::egui_glow::HardwareAcceleration::Required;
    options.glow_options.vsync = true;
    options
}

/// One backend per OS, the integrated GPU first, and only the limits the adapter offers (so
/// WARP works).
pub(crate) fn wgpu_configuration() -> WgpuConfiguration {
    let mut config =
        WgpuConfiguration::default().with_surface_config(egui_wgpu::SurfaceConfig::LOW_LATENCY);
    if let WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
        setup.instance_descriptor.backends = wgpu::Backends::from_env().unwrap_or_else(backends);
        setup.power_preference = wgpu::PowerPreference::LowPower;
        setup.native_adapter_selector = Some(Arc::new(select_adapter));
        setup.device_descriptor = Arc::new(|adapter: &wgpu::Adapter| wgpu::DeviceDescriptor {
            label: Some("pr-monitor"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        });
    }
    config
}

#[cfg(windows)]
const fn backends() -> wgpu::Backends {
    wgpu::Backends::DX12
}

#[cfg(target_os = "macos")]
const fn backends() -> wgpu::Backends {
    wgpu::Backends::METAL
}

#[cfg(all(unix, not(target_os = "macos")))]
fn backends() -> wgpu::Backends {
    wgpu::Backends::VULKAN | wgpu::Backends::GL
}

const fn adapter_rank(device: wgpu::DeviceType) -> u8 {
    match device {
        wgpu::DeviceType::IntegratedGpu => 0,
        wgpu::DeviceType::DiscreteGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

/// Integrated (keeps a laptop's discrete GPU asleep) before discrete, software last. Never
/// fails while any adapter exists.
pub(crate) fn select_adapter(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    adapters
        .iter()
        .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
        .min_by_key(|a| adapter_rank(a.get_info().device_type))
        .or_else(|| adapters.first())
        .cloned()
        .ok_or_else(|| "no graphics adapter".to_owned())
}

/// What the About page and the policy need to know about the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdapterFacts {
    pub name: String,
    pub software: bool,
}

const SOFTWARE_NAMES: [&str; 3] = ["GDI Generic", "Basic Render", "llvmpipe"];

pub(crate) fn is_software_name(name: &str) -> bool {
    SOFTWARE_NAMES.iter().any(|s| name.contains(s))
}

/// From the wgpu creation context. OpenGL reports nothing: reading its renderer string needs
/// `unsafe`, which this crate does not use (software GL falls back to wgpu via the bin anyway).
pub(crate) fn adapter_facts(cc: &eframe::CreationContext<'_>) -> Option<AdapterFacts> {
    let state = cc.wgpu_render_state.as_ref()?;
    let info = state.adapter.get_info();
    let software = info.device_type == wgpu::DeviceType::Cpu || is_software_name(&info.name);
    Some(AdapterFacts {
        name: info.name,
        software,
    })
}

#[cfg(test)]
mod tests;
