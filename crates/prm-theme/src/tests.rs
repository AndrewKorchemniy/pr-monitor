//! The built-in themes as a whole: they parse, match their file names, and pass every rule.

use crate::{
    Rgba8, ThemeRegistry, builtin::builtin_sources, contrast::relative_luminance,
    file::parse_theme, metrics::Look, self_check,
};

#[test]
fn builtin_themes_parse_without_issues() {
    for (stem, src) in builtin_sources() {
        let (_, issues) = parse_theme(src, stem).unwrap();
        assert_eq!(issues, Vec::new(), "{stem}");
    }
}

#[test]
fn builtin_themes_meet_contrast_rules() {
    let registry = ThemeRegistry::builtin();
    for (id, _) in registry.list() {
        let (_, issues) = registry.tokens(id.as_str(), Look::default()).unwrap();
        assert_eq!(issues, Vec::new(), "{id}");
    }
}

/// CIE L* (0..=100) of an opaque colour.
fn lightness(color: Rgba8) -> f64 {
    let y = relative_luminance(color);
    let f = if y > 216.0 / 24389.0 {
        y.cbrt()
    } else {
        (24389.0 / 27.0 * y + 16.0) / 116.0
    };
    116.0 * f - 16.0
}

/// Stripes must be seen: the stripe tone sits at least this far from the list tone (L*).
const MIN_STRIPE_STEP: f64 = 0.8;

#[test]
fn every_builtin_theme_has_visible_stripes() {
    let registry = ThemeRegistry::builtin();
    for (id, _) in registry.list() {
        let (tokens, _) = registry.tokens(id.as_str(), Look::default()).unwrap();
        let palette = &tokens.palette;
        let step = (lightness(palette.row_stripe) - lightness(palette.surface_sunken)).abs();
        assert!(
            step >= MIN_STRIPE_STEP,
            "{id}: stripe {} is only {step:.2} L* from the list {}",
            palette.row_stripe.to_hex(),
            palette.surface_sunken.to_hex()
        );
    }
}

#[test]
fn builtin_ids_match_file_stems() {
    for (stem, src) in builtin_sources() {
        let file: toml::Table = toml::from_str(src).unwrap();
        assert_eq!(file["id"].as_str(), Some(*stem));
    }
}

#[test]
fn builtin_has_every_shipped_theme() {
    let registry = ThemeRegistry::builtin();
    assert!(registry.get(crate::registry::DEFAULT_DARK).is_some());
    assert!(registry.get(crate::registry::DEFAULT_LIGHT).is_some());
    assert!(registry.get("cyanotype").is_some());
    assert!(registry.get("paper").is_some());
    assert!(registry.get("andromeda").is_some());
}

#[test]
fn self_check_counts_5() {
    assert_eq!(self_check(), Ok(5));
}
