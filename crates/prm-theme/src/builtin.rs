//! The themes compiled in from `themes/*.toml` (see `build.rs`), and the startup self-check.

use crate::{
    file::parse_theme,
    issue::IssueLevel,
    metrics::Density,
    tokens::{ThemeSource, Tokens},
};

include!(concat!(env!("OUT_DIR"), "/builtin_themes.rs"));

/// `(file stem, TOML text)` for every built-in theme, sorted by stem.
pub fn builtin_sources() -> &'static [(&'static str, &'static str)] {
    BUILTIN
}

/// Parses, derives and validates every built-in theme at both densities. `Ok(count)` when all
/// are clean; used by `--self-test`.
pub fn self_check() -> Result<usize, String> {
    for (stem, src) in BUILTIN {
        let (file, mut issues) = parse_theme(src, stem).map_err(|e| e.to_string())?;
        for density in [Density::Compact, Density::Comfortable] {
            let (_, built) = Tokens::build(&file, src, ThemeSource::Builtin, density);
            issues.extend(built);
        }
        if let Some(issue) = issues.iter().find(|i| i.level >= IssueLevel::Warning) {
            return Err(format!("{stem}: {}", issue.message));
        }
    }
    Ok(BUILTIN.len())
}
