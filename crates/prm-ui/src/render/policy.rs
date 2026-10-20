//! Low-power and animation policy, resolved from settings and the machine.

use prm_config::schema::Tristate;

use crate::render::RendererKind;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderFacts {
    pub renderer: RendererKind,
    /// WARP, llvmpipe, GDI Generic.
    pub software_adapter: bool,
    pub remote_session: bool,
    /// The OS "animate controls" preference, when known.
    pub os_animations: Option<bool>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderPolicy {
    /// No shadows, no caret blink, minute-granular countdown.
    pub low_power: bool,
    pub animations: bool,
}

impl RenderPolicy {
    /// Automatic: low power over remote desktops and with software rendering; animations off
    /// in low power or when the OS turned them off.
    pub(crate) fn resolve(low_power: Tristate, animations: Tristate, facts: RenderFacts) -> Self {
        let low_power = match low_power {
            Tristate::On => true,
            Tristate::Off => false,
            Tristate::Auto => facts.remote_session || facts.software_adapter,
        };
        let animations = match animations {
            Tristate::On => true,
            Tristate::Off => false,
            Tristate::Auto => !low_power && facts.os_animations != Some(false),
        };
        Self {
            low_power,
            animations,
        }
    }
}

#[cfg(test)]
mod tests;
