//! THE ONLY module that builds `Color32` values or egui `Style`/`Visuals` from theme tokens.
//! Everything else reads colors from `UiColors`.

use eframe::egui::{
    self, Color32, CornerRadius, Margin, Shadow, Stroke, Style, Visuals, style::WidgetVisuals,
};
use prm_domain::Vsid;
use prm_theme::{
    Appearance, Palette, Rgba8,
    contrast::ensure_contrast,
    oklab::{Oklab, from_oklab},
};

use crate::{
    render::policy::RenderPolicy,
    theme::{Tone, ToneColors, UiTheme},
};

/// A spacing token as an egui margin (tokens are small whole points).
#[expect(
    clippy::cast_possible_truncation,
    reason = "spacing tokens are 2..=32 points"
)]
fn px(points: f32) -> i8 {
    points.round().clamp(0.0, f32::from(i8::MAX)) as i8
}

pub(crate) fn color(c: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Declares `UiColors` with one `Color32` per palette key.
macro_rules! define_ui_colors {
    ($(($field:ident, $key:literal)),* $(,)?) => {
        /// The palette as egui colors, built once per theme.
        #[derive(Clone, Debug)]
        #[expect(dead_code, reason = "one field per palette key; not every key is painted yet")]
        pub(crate) struct UiColors {
            $(pub $field: Color32,)*
        }

        impl From<&prm_theme::Palette> for UiColors {
            fn from(palette: &prm_theme::Palette) -> Self {
                Self { $($field: color(palette.$field),)* }
            }
        }
    };
}

prm_theme::for_each_palette_key!(define_ui_colors);

impl UiColors {
    pub(crate) const fn tone(&self, tone: Tone) -> ToneColors {
        let (solid, bg, fg) = match tone {
            Tone::NeedsReview => (
                self.status_needs_review_solid,
                self.status_needs_review_bg,
                self.status_needs_review_fg,
            ),
            Tone::ActiveComments => (
                self.status_active_comments_solid,
                self.status_active_comments_bg,
                self.status_active_comments_fg,
            ),
            Tone::Reviewed => (
                self.status_reviewed_solid,
                self.status_reviewed_bg,
                self.status_reviewed_fg,
            ),
            Tone::Danger => (
                self.feedback_danger_solid,
                self.feedback_danger_bg,
                self.feedback_danger_fg,
            ),
            Tone::Warning => (
                self.feedback_warning_solid,
                self.feedback_warning_bg,
                self.feedback_warning_fg,
            ),
            Tone::Success => (
                self.feedback_success_solid,
                self.feedback_success_bg,
                self.feedback_success_fg,
            ),
            Tone::Info => (
                self.feedback_info_solid,
                self.feedback_info_bg,
                self.feedback_info_fg,
            ),
            Tone::Neutral => (self.text_muted, self.control_bg, self.text_secondary),
        };
        ToneColors { solid, bg, fg }
    }
}

/// The full egui style for one theme.
pub(crate) fn style(theme: &UiTheme, policy: RenderPolicy) -> Style {
    let m = &theme.tokens.metrics;
    let mut style = Style {
        text_styles: [
            (egui::TextStyle::Small, theme.fonts.small.clone()),
            (egui::TextStyle::Body, theme.fonts.body.clone()),
            (egui::TextStyle::Button, theme.fonts.button.clone()),
            (egui::TextStyle::Heading, theme.fonts.title.clone()),
            (egui::TextStyle::Monospace, theme.fonts.mono.clone()),
        ]
        .into(),
        animation_time: if policy.animations {
            m.motion.quick
        } else {
            0.0
        },
        ..Style::default()
    };
    if !policy.animations {
        style.scroll_animation = egui::style::ScrollAnimation::none();
    }
    style.explanation_tooltips = false;
    style.url_in_tooltip = false;
    style.always_scroll_the_only_direction = true;
    style.compact_menu_style = true;
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(m.space.md, m.space.sm);
    spacing.button_padding = egui::vec2(m.space.lg + 2.0, m.space.sm);
    spacing.interact_size = egui::vec2(m.space.xxxl, theme.control_height());
    spacing.menu_margin = Margin::same(px(m.space.xs));
    spacing.window_margin = Margin::same(px(m.space.lg));
    spacing.indent = m.space.xl;
    spacing.icon_width = 14.0;
    spacing.icon_width_inner = 8.0;
    spacing.icon_spacing = m.space.sm;
    spacing.tooltip_width = 420.0;
    spacing.menu_width = 240.0;
    spacing.combo_height = 320.0;
    spacing.scroll = no_spring_back(egui::style::ScrollStyle::thin());
    style.interaction.tooltip_delay = 0.45;
    style.interaction.selectable_labels = false;
    style.visuals = visuals(theme, policy);
    style
}

fn widget(theme: &UiTheme, fill: Color32, stroke: Color32, fg: Color32) -> WidgetVisuals {
    let m = &theme.tokens.metrics;
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(m.stroke.hairline, stroke),
        corner_radius: CornerRadius::same(m.radius.md),
        fg_stroke: Stroke::new(1.0, fg),
        expansion: 0.0,
    }
}

fn visuals(theme: &UiTheme, policy: RenderPolicy) -> Visuals {
    let (m, c) = (&theme.tokens.metrics, &theme.colors);
    let dark = theme.tokens.appearance == Appearance::Dark;
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.dark_mode = dark;
    v.override_text_color = Some(c.text_primary);
    v.weak_text_color = Some(c.text_muted);
    v.hyperlink_color = c.text_link;
    v.faint_bg_color = c.row_stripe;
    v.extreme_bg_color = c.surface_sunken;
    v.text_edit_bg_color = Some(c.surface_sunken);
    v.code_bg_color = c.surface_raised;
    v.warn_fg_color = c.feedback_warning_fg;
    v.error_fg_color = c.feedback_danger_fg;
    v.panel_fill = c.surface_canvas;
    v.window_fill = c.surface_raised;
    v.window_stroke = Stroke::new(m.stroke.hairline, c.border_default);
    v.window_corner_radius = CornerRadius::same(m.radius.lg);
    v.menu_corner_radius = CornerRadius::same(m.radius.md);
    let shadow = if policy.low_power {
        Shadow::NONE
    } else {
        Shadow {
            offset: m.elevation.popup_offset,
            blur: m.elevation.popup_blur,
            spread: 0,
            color: c.effect_shadow,
        }
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.selection.bg_fill = c.selection_text_bg;
    v.selection.stroke = Stroke::new(1.0, c.border_focus);
    v.widgets.noninteractive = widget(theme, c.surface_canvas, c.border_subtle, c.text_primary);
    v.widgets.inactive = widget(theme, c.control_bg, c.border_default, c.text_primary);
    v.widgets.hovered = widget(theme, c.control_hover, c.border_focus, c.text_primary);
    v.widgets.active = widget(theme, c.control_active, c.border_focus, c.text_primary);
    v.widgets.open = widget(theme, c.control_hover, c.border_control, c.text_primary);
    v.striped = false;
    v.button_frame = true;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    v.window_highlight_topmost = false;
    v.image_loading_spinners = false;
    v.disabled_alpha = 0.5;
    v.text_cursor.stroke = Stroke::new(2.0, c.accent_base);
    v.text_cursor.blink = !policy.low_power;
    v
}

/// Chrome bars (toolbar, status bar).
pub(crate) fn frame_chrome(theme: &UiTheme) -> egui::Frame {
    let m = &theme.tokens.metrics;
    egui::Frame::NONE
        .fill(theme.colors.surface_canvas)
        .inner_margin(Margin::symmetric(px(m.space.md), 0))
}

/// A scroll bar that floats over the content, so the content keeps its width when it starts to
/// scroll, without spring-back (see `no_spring_back`).
pub(crate) fn floating_scroll() -> egui::style::ScrollStyle {
    no_spring_back(egui::style::ScrollStyle::floating())
}

/// `style` without egui's rubber-band spring-back. When content got shorter at its end (a group
/// collapsed, a refresh, wrapped text settling), the offset was past the new end and sprang back
/// over a dozen frames, which read as the content bouncing at the end of a touchpad fling. This
/// app has no gesture that pulls past the edge, so the offset simply stays within the content.
fn no_spring_back(mut style: egui::style::ScrollStyle) -> egui::style::ScrollStyle {
    style.kinetic.rubber_band = false;
    style
}

/// The peek drawer. No border: the panel's separator draws its left edge, and the toolbar, status bar and
/// window bound the rest, so a stroke here doubled those hairlines.
pub(crate) fn frame_raised(theme: &UiTheme) -> egui::Frame {
    let m = &theme.tokens.metrics;
    egui::Frame::NONE
        .fill(theme.colors.surface_raised)
        .inner_margin(Margin::same(px(m.space.lg)))
}

/// A block set into the peek (an open thread), padded and rounded like the controls around it.
pub(crate) fn frame_sunken(theme: &UiTheme) -> egui::Frame {
    let m = &theme.tokens.metrics;
    egui::Frame::NONE
        .fill(theme.colors.surface_sunken)
        .inner_margin(Margin::symmetric(px(m.space.md), px(m.space.sm)))
        .corner_radius(CornerRadius::same(m.radius.md))
}

/// The window background behind content (the dashboard body, settings pages).
pub(crate) fn frame_canvas(theme: &UiTheme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.colors.surface_canvas)
}

/// A standalone card with a hairline border, like a dialog (first run).
pub(crate) fn frame_card(theme: &UiTheme) -> egui::Frame {
    let m = &theme.tokens.metrics;
    egui::Frame::NONE
        .fill(theme.colors.surface_raised)
        .stroke(Stroke::new(m.stroke.hairline, theme.colors.border_default))
        .corner_radius(CornerRadius::same(m.radius.lg))
        .inner_margin(Margin::same(px(m.space.xxl)))
}

/// A banner strip in the given tone.
pub(crate) fn frame_banner(theme: &UiTheme, tone: Tone) -> egui::Frame {
    let m = &theme.tokens.metrics;
    let colors = theme.colors.tone(tone);
    egui::Frame::NONE
        .fill(colors.bg)
        .inner_margin(Margin::symmetric(px(m.space.lg), px(m.space.sm)))
}

/// Popups: palette, menus, modals.
pub(crate) fn frame_popup(theme: &UiTheme, policy: RenderPolicy) -> egui::Frame {
    let v = visuals(theme, policy);
    egui::Frame::NONE
        .fill(theme.colors.surface_overlay)
        .stroke(Stroke::new(
            theme.tokens.metrics.stroke.hairline,
            theme.colors.border_default,
        ))
        .corner_radius(CornerRadius::same(theme.tokens.metrics.radius.lg))
        .shadow(v.popup_shadow)
        .inner_margin(Margin::same(px(theme.tokens.metrics.space.lg)))
}

/// Dialogs: the popup surface with roomier padding and a deeper shadow.
pub(crate) fn frame_dialog(theme: &UiTheme, policy: RenderPolicy) -> egui::Frame {
    let m = &theme.tokens.metrics;
    let mut frame =
        frame_popup(theme, policy).inner_margin(Margin::same(px(m.space.xl + m.space.xs)));
    if !policy.low_power {
        frame.shadow.blur = frame.shadow.blur.saturating_add(8);
        frame.shadow.offset = [0, m.elevation.popup_offset[1].saturating_mul(2)];
    }
    frame
}

/// How many hues the initials circles are spread over.
pub(crate) const AVATAR_HUES: usize = 24;
/// Initials must read as well as body text.
const AVATAR_MIN_CONTRAST: f64 = 4.5;

/// An initials circle's fill and text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvatarTint {
    pub bg: Color32,
    pub text: Color32,
}

/// Which of the `AVATAR_HUES` a person's initials circle uses. Stable across runs and machines
/// (it is derived from the id alone), so the same person keeps the same colour.
pub(crate) fn avatar_hue(id: Vsid) -> usize {
    let bucket = prm_domain::hash::fnv64(id.as_bytes()) % AVATAR_HUES as u64;
    usize::try_from(bucket).unwrap_or(0)
}

/// The circle colours for every hue: a muted fill (dark in dark themes, pale in light ones)
/// with the theme's primary text colour moved just far enough to hold `AVATAR_MIN_CONTRAST`.
pub(crate) fn avatar_tints(palette: &Palette, dark: bool) -> [AvatarTint; AVATAR_HUES] {
    let lightness = if dark { 0.42 } else { 0.88 };
    std::array::from_fn(|bucket| {
        let hue = (bucket as f64 * 360.0 / AVATAR_HUES as f64).to_radians();
        let chroma = 0.08;
        let lab = Oklab {
            l: lightness,
            a: chroma * hue.cos(),
            b: chroma * hue.sin(),
        };
        let bg = from_oklab(lab, u8::MAX);
        let text = ensure_contrast(palette.text_primary, bg, AVATAR_MIN_CONTRAST);
        AvatarTint {
            bg: color(bg),
            text: color(text),
        }
    })
}

/// Ensures nothing outside this module builds colors from literals.
#[cfg(test)]
mod tests;
