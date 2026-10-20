//! The theme at run time: resolves the user's selection to dark and light `UiTheme`s, applies
//! them as egui styles when they change, and makes the current one available to every widget.

pub(crate) mod adapter;
pub(crate) mod fonts;
pub(crate) mod icons;

use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

use eframe::egui::{self, Color32, FontId};
use prm_config::{UiPrefs, schema::Density as ConfigDensity};
use prm_theme::{
    Appearance, Density, MarkColors, ThemeDirScanner, ThemeIssue, ThemeRegistry, ThemeSelection,
    Tokens, TypeRole, registry::resolve,
};

use crate::{deps::ChromeColors, render::policy::RenderPolicy};

/// The semantic tones status pills, banners and chips use.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    NeedsReview,
    ActiveComments,
    Reviewed,
    Danger,
    Warning,
    Success,
    Info,
    Neutral,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToneColors {
    pub solid: Color32,
    pub bg: Color32,
    pub fg: Color32,
}

/// One `FontId` per type role.
#[derive(Clone, Debug)]
pub(crate) struct FontIds {
    pub display: FontId,
    pub title: FontId,
    pub body: FontId,
    pub body_strong: FontId,
    pub button: FontId,
    pub label: FontId,
    pub small: FontId,
    pub mono: FontId,
    pub mono_strong: FontId,
}

impl FontIds {
    fn new() -> Self {
        let id = |role: TypeRole| fonts::font_id(&role.spec());
        Self {
            display: id(TypeRole::Display),
            title: id(TypeRole::Title),
            body: id(TypeRole::Body),
            body_strong: id(TypeRole::BodyStrong),
            button: id(TypeRole::Button),
            label: id(TypeRole::Label),
            small: id(TypeRole::Small),
            mono: id(TypeRole::Mono),
            mono_strong: id(TypeRole::MonoStrong),
        }
    }
}

/// A theme ready for painting.
#[derive(Clone, Debug)]
pub(crate) struct UiTheme {
    pub tokens: Arc<Tokens>,
    pub colors: adapter::UiColors,
    pub fonts: FontIds,
    pub policy: RenderPolicy,
}

impl UiTheme {
    pub(crate) fn new(tokens: Arc<Tokens>, policy: RenderPolicy) -> Self {
        Self {
            colors: adapter::UiColors::from(&tokens.palette),
            fonts: FontIds::new(),
            tokens,
            policy,
        }
    }

    pub(crate) fn is_dark(&self) -> bool {
        self.tokens.appearance == Appearance::Dark
    }
}

/// Graphite, for code that runs before a theme was installed (tests, first pass).
pub(crate) fn fallback() -> Arc<UiTheme> {
    static FALLBACK: OnceLock<Arc<UiTheme>> = OnceLock::new();
    FALLBACK
        .get_or_init(|| {
            let registry = ThemeRegistry::builtin();
            let selection = ThemeSelection {
                theme: prm_theme::registry::DEFAULT_DARK.into(),
                dark: prm_theme::registry::DEFAULT_DARK.into(),
                light: prm_theme::registry::DEFAULT_LIGHT.into(),
                density: Density::Compact,
            };
            let set = resolve(&registry, &selection);
            let policy = RenderPolicy {
                low_power: false,
                animations: true,
            };
            Arc::new(UiTheme::new(set.dark, policy))
        })
        .clone()
}

/// The one conversion from config prefs to a theme selection.
pub(crate) fn selection_from(prefs: &UiPrefs) -> ThemeSelection {
    ThemeSelection {
        theme: prefs.theme.clone(),
        dark: prefs.theme_dark.clone(),
        light: prefs.theme_light.clone(),
        density: match prefs.density {
            ConfigDensity::Comfortable => Density::Comfortable,
            ConfigDensity::Compact => Density::Compact,
        },
    }
}

pub(crate) struct ThemeRuntime {
    registry: ThemeRegistry,
    scanner: ThemeDirScanner,
    themes_dir: std::path::PathBuf,
    selection: ThemeSelection,
    preview: Option<String>,
    policy: RenderPolicy,
    dark: Arc<UiTheme>,
    light: Arc<UiTheme>,
    follow_system: bool,
    /// Bumped whenever the themes or policy change; styles are reapplied once per change.
    pub generation: u64,
    applied: u64,
    pub issues: Vec<ThemeIssue>,
}

impl ThemeRuntime {
    pub(crate) fn new(themes_dir: &Path, prefs: &UiPrefs, policy: RenderPolicy) -> Self {
        let (registry, mut issues) = ThemeRegistry::builtin().with_user_dir(themes_dir);
        let selection = selection_from(prefs);
        let set = resolve(&registry, &selection);
        issues.extend(set.issues);
        Self {
            scanner: ThemeDirScanner::new(themes_dir.to_path_buf()),
            themes_dir: themes_dir.to_path_buf(),
            dark: Arc::new(UiTheme::new(set.dark, policy)),
            light: Arc::new(UiTheme::new(set.light, policy)),
            follow_system: set.follow_system,
            registry,
            selection,
            preview: None,
            policy,
            generation: 1,
            applied: 0,
            issues,
        }
    }

    fn rebuild(&mut self) {
        let mut selection = self.selection.clone();
        if let Some(preview) = &self.preview {
            selection.theme.clone_from(preview);
        }
        let set = resolve(&self.registry, &selection);
        self.issues = set.issues;
        self.dark = Arc::new(UiTheme::new(set.dark, self.policy));
        self.light = Arc::new(UiTheme::new(set.light, self.policy));
        self.follow_system = set.follow_system;
        self.generation += 1;
    }

    pub(crate) fn set_selection(&mut self, selection: ThemeSelection) {
        if selection != self.selection {
            self.selection = selection;
            self.rebuild();
        }
    }

    pub(crate) fn set_policy(&mut self, policy: RenderPolicy) {
        if policy != self.policy {
            self.policy = policy;
            self.rebuild();
        }
    }

    pub(crate) const fn policy(&self) -> RenderPolicy {
        self.policy
    }

    /// Settings' live preview; `None` reverts.
    pub(crate) fn set_preview(&mut self, theme: Option<String>) {
        if theme != self.preview {
            self.preview = theme;
            self.rebuild();
        }
    }

    /// Reloads user themes when the folder changed (checked when the window gains focus).
    pub(crate) fn poll_user_dir(&mut self) {
        if self.scanner.changed() {
            self.reload();
        }
    }

    pub(crate) fn reload(&mut self) {
        let (registry, issues) = ThemeRegistry::builtin().with_user_dir(&self.themes_dir);
        self.registry = registry;
        self.rebuild();
        self.issues.extend(issues);
    }

    pub(crate) fn registry(&self) -> &ThemeRegistry {
        &self.registry
    }

    pub(crate) fn themes_dir(&self) -> &Path {
        &self.themes_dir
    }

    /// Applies egui styles once per change.
    pub(crate) fn apply(&mut self, ctx: &egui::Context) {
        if self.applied == self.generation {
            return;
        }
        ctx.set_style_of(egui::Theme::Dark, adapter::style(&self.dark, self.policy));
        ctx.set_style_of(egui::Theme::Light, adapter::style(&self.light, self.policy));
        let preference = if self.follow_system {
            egui::ThemePreference::System
        } else if self.dark.is_dark() {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        };
        ctx.set_theme(preference);
        self.applied = self.generation;
    }

    pub(crate) fn current(&self, ctx: &egui::Context) -> &Arc<UiTheme> {
        match ctx.theme() {
            egui::Theme::Dark => &self.dark,
            egui::Theme::Light => &self.light,
        }
    }

    /// Makes the current theme available to widgets for this frame.
    pub(crate) fn install_frame(&self, ctx: &egui::Context) {
        let theme = self.current(ctx).clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, theme));
    }

    pub(crate) fn chrome(&self, ctx: &egui::Context) -> ChromeColors {
        let palette = &self.current(ctx).tokens.palette;
        let rgb = |c: prm_theme::Rgba8| [c.r, c.g, c.b];
        ChromeColors {
            caption: rgb(palette.chrome_caption),
            text: rgb(palette.chrome_caption_text),
            border: rgb(palette.chrome_border),
        }
    }

    /// Tray mark colors for a badge.
    pub(crate) fn mark_colors(
        &self,
        ctx: &egui::Context,
        badge: prm_domain::view::TrayBadge,
    ) -> MarkColors {
        let palette = &self.current(ctx).tokens.palette;
        MarkColors::from_palette(palette, crate::tray::bar_color(palette, badge))
    }
}
