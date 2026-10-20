//! The palette: every semantic color key, listed exactly once in `for_each_palette_key!`.

use crate::color::Rgba8;

/// Calls `$m!` with every palette key as `(field_name, "dotted.key")`.
///
/// why a macro: the same 63-key list drives the `Palette` struct, its by-name lookup, and the
/// UI's egui-typed mirror in prm-ui. Written out by hand, those three copies would drift; a
/// function cannot declare struct fields.
#[macro_export]
macro_rules! for_each_palette_key {
    ($m:ident) => {
        $m! {
            (surface_canvas, "surface.canvas"),
            (surface_sunken, "surface.sunken"),
            (surface_raised, "surface.raised"),
            (surface_overlay, "surface.overlay"),
            (surface_scrim, "surface.scrim"),
            (row_stripe, "row.stripe"),
            (row_hover, "row.hover"),
            (row_selected, "row.selected"),
            (row_selected_inactive, "row.selected_inactive"),
            (border_subtle, "border.subtle"),
            (border_default, "border.default"),
            (border_control, "border.control"),
            (border_focus, "border.focus"),
            (text_primary, "text.primary"),
            (text_secondary, "text.secondary"),
            (text_muted, "text.muted"),
            (text_disabled, "text.disabled"),
            (text_on_accent, "text.on_accent"),
            (text_link, "text.link"),
            (accent_base, "accent.base"),
            (accent_hover, "accent.hover"),
            (accent_pressed, "accent.pressed"),
            (accent_subtle, "accent.subtle"),
            (accent_text, "accent.text"),
            (control_bg, "control.bg"),
            (control_hover, "control.hover"),
            (control_active, "control.active"),
            (status_needs_review_solid, "status.needs_review.solid"),
            (status_needs_review_bg, "status.needs_review.bg"),
            (status_needs_review_fg, "status.needs_review.fg"),
            (status_active_comments_solid, "status.active_comments.solid"),
            (status_active_comments_bg, "status.active_comments.bg"),
            (status_active_comments_fg, "status.active_comments.fg"),
            (status_reviewed_solid, "status.reviewed.solid"),
            (status_reviewed_bg, "status.reviewed.bg"),
            (status_reviewed_fg, "status.reviewed.fg"),
            (feedback_danger_solid, "feedback.danger.solid"),
            (feedback_danger_bg, "feedback.danger.bg"),
            (feedback_danger_fg, "feedback.danger.fg"),
            (feedback_warning_solid, "feedback.warning.solid"),
            (feedback_warning_bg, "feedback.warning.bg"),
            (feedback_warning_fg, "feedback.warning.fg"),
            (feedback_success_solid, "feedback.success.solid"),
            (feedback_success_bg, "feedback.success.bg"),
            (feedback_success_fg, "feedback.success.fg"),
            (feedback_info_solid, "feedback.info.solid"),
            (feedback_info_bg, "feedback.info.bg"),
            (feedback_info_fg, "feedback.info.fg"),
            (vote_approved, "vote.approved"),
            (vote_suggestions, "vote.suggestions"),
            (vote_waiting, "vote.waiting"),
            (vote_rejected, "vote.rejected"),
            (vote_none, "vote.none"),
            (signature_revision, "signature.revision"),
            (selection_text_bg, "selection.text_bg"),
            (effect_shadow, "effect.shadow"),
            (chrome_caption, "chrome.caption"),
            (chrome_caption_text, "chrome.caption_text"),
            (chrome_border, "chrome.border"),
            (tray_body, "tray.body"),
            (tray_outline, "tray.outline"),
            (tray_lines, "tray.lines"),
            (tray_idle, "tray.idle"),
        }
    };
}

/// Declares `Palette` and its by-key access from the key list.
macro_rules! define_palette {
    ($(($field:ident, $key:literal)),* $(,)?) => {
        /// Every semantic color of a theme. Built in one struct literal by `derive_palette`, so
        /// a missing key is a compile error.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Palette {
            $(pub $field: Rgba8,)*
        }

        impl Palette {
            /// Every key, in declaration order (also the order of `iter`).
            pub const KEYS: &'static [&'static str] = &[$($key),*];

            pub fn get(&self, key: &str) -> Option<Rgba8> {
                match key {
                    $($key => Some(self.$field),)*
                    _ => None,
                }
            }

            /// `false` when the key is unknown.
            pub fn set(&mut self, key: &str, value: Rgba8) -> bool {
                match key {
                    $($key => {
                        self.$field = value;
                        true
                    })*
                    _ => false,
                }
            }

            pub fn iter(&self) -> impl Iterator<Item = (&'static str, Rgba8)> + '_ {
                [$(($key, self.$field)),*].into_iter()
            }
        }
    };
}

for_each_palette_key!(define_palette);

#[cfg(test)]
mod tests;
