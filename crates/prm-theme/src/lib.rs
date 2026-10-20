//! Themes, independent of any UI toolkit: theme files (seeds + overrides), the OKLab derivation
//! of a full semantic palette, WCAG validation, metrics, the theme registry, and the brand mark.
//! Adding a theme is adding one TOML file.

pub mod builtin;
pub mod color;
pub mod contrast;
pub mod derive;
pub mod file;
pub mod issue;
pub mod mark;
pub mod metrics;
pub mod oklab;
pub mod palette;
pub mod registry;
pub mod scan;
pub mod tokens;
pub mod validate;

pub use builtin::self_check;
pub use color::Rgba8;
pub use file::{Appearance, ThemeFile};
pub use issue::{IssueLevel, ThemeId, ThemeIssue};
pub use mark::{MarkColors, render_mark};
pub use metrics::{Density, Metrics, TypeRole};
pub use palette::Palette;
pub use registry::{ThemeRegistry, ThemeSelection, ThemeSet, resolve};
pub use scan::ThemeDirScanner;
pub use tokens::{ThemeSource, Tokens};

#[cfg(test)]
mod tests;
