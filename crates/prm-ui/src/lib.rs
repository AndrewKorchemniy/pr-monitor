//! prm-ui: the egui dashboard, settings, tray and window behaviour. It knows nothing of the
//! platform; the bin bridges platform services through [`UiHost`].
#![forbid(unsafe_code)]

mod actions;
mod app;
mod commands;
mod copy;
mod deps;
mod format;
mod frame;
mod link;
mod render;
mod route;
mod screens;
mod theme;
mod tray;
mod ui_state;
mod view;
mod widgets;
mod window;

pub use app::create_app;
pub use deps::{BuildInfo, NoopHost, UiHost, UiInit, WindowChrome};
pub use render::{LaunchSpec, RenderEnv, RendererKind, hover_bands::HoverBands, native_options};
pub use tray::window_icon;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
