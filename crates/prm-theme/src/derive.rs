//! Seeds → the full palette, in OKLab, with every text and mark contrast-guarded.

use crate::{
    color::Rgba8,
    contrast::{contrast_ratio, ensure_contrast as ensure},
    file::{Appearance, ThemeFile},
    oklab::{lift, mix},
    palette::Palette,
};

/// WCAG minimums the derivation guarantees (the validator checks the same numbers).
const TEXT_STRONG: f64 = 7.0;
const TEXT: f64 = 4.5;
const NON_TEXT: f64 = 3.0;

/// The seed-derived values every key is computed from.
struct Basis {
    dark: bool,
    base: Rgba8,
    ink: Rgba8,
    accent: Rgba8,
    contrast: f64,
    /// `surface.sunken`: the table background, the reference for most contrast checks.
    sunken: Rgba8,
    row_hover: Rgba8,
    row_selected: Rgba8,
}

impl Basis {
    const fn pick(&self, dark: f64, light: f64) -> f64 {
        if self.dark { dark } else { light }
    }

    /// Stronger borders and secondary text as `contrast` rises.
    fn k(&self, x: f64) -> f64 {
        x * self.contrast.mul_add(0.6, 1.0)
    }

    fn base_ink(&self, t: f64) -> Rgba8 {
        mix(self.base, self.ink, t)
    }
}

/// One status or feedback tone: a solid mark, a tinted background and readable text on it.
struct Tone {
    solid: Rgba8,
    bg: Rgba8,
    fg: Rgba8,
}

impl Tone {
    fn new(seed: Rgba8, b: &Basis) -> Self {
        let solid = ensure(ensure(seed, b.sunken, NON_TEXT), b.row_selected, NON_TEXT);
        let bg = mix(
            b.sunken,
            seed,
            b.pick(0.14, 0.11) * b.contrast.mul_add(-0.3, 1.0),
        );
        let fg = ensure(seed, bg, TEXT);
        Self { solid, bg, fg }
    }
}

fn basis(file: &ThemeFile) -> Basis {
    let seeds = &file.seeds;
    let dark = file.appearance == Appearance::Dark;
    let sunken = lift(seeds.base, if dark { -0.025 } else { 0.030 });
    Basis {
        dark,
        base: seeds.base,
        ink: seeds.ink,
        accent: seeds.accent,
        contrast: f64::from(seeds.contrast),
        sunken,
        row_hover: mix(sunken, seeds.ink, 0.060),
        row_selected: mix(sunken, seeds.accent, if dark { 0.20 } else { 0.14 }),
    }
}

/// First of base and ink that reads on the accent; otherwise black or white.
fn on_accent(b: &Basis) -> Rgba8 {
    [b.base, b.ink]
        .into_iter()
        .find(|c| contrast_ratio(*c, b.accent) >= TEXT)
        .unwrap_or_else(|| {
            let black = contrast_ratio(Rgba8::BLACK, b.accent);
            let white = contrast_ratio(Rgba8::WHITE, b.accent);
            if black >= white {
                Rgba8::BLACK
            } else {
                Rgba8::WHITE
            }
        })
}

/// The whole palette in one struct literal, so a missing key cannot compile.
#[expect(
    clippy::too_many_lines,
    reason = "one line per palette key; splitting the literal would lose the completeness check"
)]
pub fn derive_palette(file: &ThemeFile) -> Palette {
    let b = basis(file);
    let seeds = &file.seeds;
    let status = &seeds.status;
    let feedback = &seeds.feedback;
    let raised = lift(b.base, b.pick(0.045, 0.040));
    let needs_review = Tone::new(status.needs_review, &b);
    let active = Tone::new(status.active_comments, &b);
    let reviewed = Tone::new(status.reviewed, &b);
    let danger = Tone::new(feedback.danger, &b);
    let warning = Tone::new(feedback.warning, &b);
    let success = Tone::new(feedback.success.unwrap_or(status.reviewed), &b);
    let info = Tone::new(feedback.info.unwrap_or(b.accent), &b);
    let secondary = ensure(
        ensure(
            b.base_ink(b.contrast.mul_add(0.12, 0.74)),
            b.row_selected,
            TEXT,
        ),
        raised,
        TEXT,
    );
    let muted = ensure(
        b.base_ink(b.contrast.mul_add(0.16, 0.56)),
        b.row_hover,
        NON_TEXT,
    );
    let accent_text = ensure(b.accent, b.sunken, TEXT);
    let border_subtle = b.base_ink(b.k(0.10));
    Palette {
        surface_canvas: b.base,
        surface_sunken: b.sunken,
        surface_raised: raised,
        surface_overlay: lift(b.base, b.pick(0.070, 0.040)),
        surface_scrim: Rgba8::BLACK.with_alpha(alpha(b.pick(0.55, 0.32))),
        row_stripe: mix(b.sunken, b.ink, 0.022),
        row_hover: b.row_hover,
        row_selected: b.row_selected,
        row_selected_inactive: mix(b.sunken, b.ink, 0.08),
        border_subtle,
        border_default: b.base_ink(b.k(0.17)),
        border_control: ensure_both(b.base_ink(b.k(0.32)), &b),
        border_focus: ensure_both(b.accent, &b),
        text_primary: ensure(b.ink, b.sunken, TEXT_STRONG),
        text_secondary: secondary,
        text_muted: muted,
        text_disabled: b.base_ink(0.38),
        text_on_accent: on_accent(&b),
        text_link: accent_text,
        accent_base: b.accent,
        accent_hover: lift(b.accent, b.pick(0.05, -0.05)),
        accent_pressed: lift(b.accent, b.pick(-0.04, -0.10)),
        accent_subtle: mix(b.sunken, b.accent, b.pick(0.16, 0.12)),
        accent_text,
        control_bg: b.base_ink(0.06),
        control_hover: b.base_ink(0.10),
        control_active: b.base_ink(0.15),
        status_needs_review_solid: needs_review.solid,
        status_needs_review_bg: needs_review.bg,
        status_needs_review_fg: needs_review.fg,
        status_active_comments_solid: active.solid,
        status_active_comments_bg: active.bg,
        status_active_comments_fg: active.fg,
        status_reviewed_solid: reviewed.solid,
        status_reviewed_bg: reviewed.bg,
        status_reviewed_fg: reviewed.fg,
        feedback_danger_solid: danger.solid,
        feedback_danger_bg: danger.bg,
        feedback_danger_fg: danger.fg,
        feedback_warning_solid: warning.solid,
        feedback_warning_bg: warning.bg,
        feedback_warning_fg: warning.fg,
        feedback_success_solid: success.solid,
        feedback_success_bg: success.bg,
        feedback_success_fg: success.fg,
        feedback_info_solid: info.solid,
        feedback_info_bg: info.bg,
        feedback_info_fg: info.fg,
        vote_approved: reviewed.solid,
        vote_suggestions: reviewed.solid,
        vote_waiting: warning.solid,
        vote_rejected: danger.solid,
        vote_none: muted,
        signature_revision: ensure(
            ensure(b.accent, b.row_selected, NON_TEXT),
            b.row_hover,
            NON_TEXT,
        ),
        selection_text_bg: mix(b.sunken, b.accent, 0.35),
        effect_shadow: Rgba8::BLACK.with_alpha(alpha(b.pick(0.45, 0.16))),
        chrome_caption: b.base,
        chrome_caption_text: b.ink,
        chrome_border: border_subtle,
        tray_body: b.base,
        tray_outline: b.ink,
        tray_lines: secondary,
        tray_idle: muted,
    }
}

/// Readable on both the table (sunken) and the window (canvas), as validation requires.
fn ensure_both(color: Rgba8, b: &Basis) -> Rgba8 {
    ensure(ensure(color, b.sunken, NON_TEXT), b.base, NON_TEXT)
}

fn alpha(fraction: f64) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a 0..=1 fraction scaled to 0..=255"
    )]
    let byte = (fraction.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

#[cfg(test)]
mod tests;
