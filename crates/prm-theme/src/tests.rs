//! The built-in themes as a whole: they parse, match their file names, and pass every rule.

use crate::{Density, ThemeRegistry, builtin::builtin_sources, file::parse_theme, self_check};

#[test]
fn builtin_themes_parse_without_issues() {
    for (stem, src) in builtin_sources() {
        let (_, issues) = parse_theme(src, stem).unwrap();
        assert_eq!(issues, Vec::new(), "{stem}");
    }
}

#[test]
fn builtin_themes_meet_contrast_rules_both_densities() {
    let registry = ThemeRegistry::builtin();
    for (id, _) in registry.list() {
        for density in [Density::Compact, Density::Comfortable] {
            let (_, issues) = registry.tokens(id.as_str(), density).unwrap();
            assert_eq!(issues, Vec::new(), "{id} {density:?}");
        }
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
fn builtin_has_graphite_and_vellum() {
    let registry = ThemeRegistry::builtin();
    assert!(registry.get(crate::registry::DEFAULT_DARK).is_some());
    assert!(registry.get(crate::registry::DEFAULT_LIGHT).is_some());
    assert!(registry.get("cyanotype").is_some());
}

#[test]
fn self_check_counts_3() {
    assert_eq!(self_check(), Ok(3));
}
