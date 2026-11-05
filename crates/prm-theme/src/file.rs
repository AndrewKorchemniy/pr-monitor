//! The theme file format (`themes/*.toml`): a few seeds and per-key overrides.
//! Unknown keys are ignored, like config.toml.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    color::Rgba8,
    issue::{IssueLevel, ThemeId, ThemeIssue},
};

/// The newest theme file schema this build understands.
pub const THEME_SCHEMA: u32 = 1;
/// The Info issue for a theme that still sets the retired `radius_scale`.
pub const RADIUS_SCALE_RETIRED: &str =
    "radius_scale is no longer used; use Settings › Appearance › Corners";
/// The Info issue for a theme that still sets the retired `stripes`.
pub const STRIPES_RETIRED: &str =
    "stripes is no longer a theme setting; use Settings › Appearance › Striped table rows";
/// Keys a theme's `[metrics]` table used to take, each with the Info issue it now raises.
const RETIRED_METRICS_KEYS: [(&str, &str); 2] = [
    ("radius_scale", RADIUS_SCALE_RETIRED),
    ("stripes", STRIPES_RETIRED),
];
/// `contrast` is clamped to this range.
pub const CONTRAST_RANGE: std::ops::RangeInclusive<f32> = -1.0..=1.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeFile {
    #[serde(default = "current_schema")]
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    #[serde(default)]
    pub description: String,
    pub seeds: Seeds,
    /// Palette key → color, applied after derivation.
    #[serde(default)]
    pub overrides: BTreeMap<String, Rgba8>,
}

const fn current_schema() -> u32 {
    THEME_SCHEMA
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seeds {
    pub base: Rgba8,
    pub ink: Rgba8,
    pub accent: Rgba8,
    /// -1..=1: raises (or lowers) borders and secondary text.
    #[serde(default)]
    pub contrast: f32,
    pub status: StatusSeeds,
    pub feedback: FeedbackSeeds,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusSeeds {
    pub needs_review: Rgba8,
    pub active_comments: Rgba8,
    pub reviewed: Rgba8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedbackSeeds {
    pub danger: Rgba8,
    pub warning: Rgba8,
    /// Defaults to the reviewed status seed.
    #[serde(default)]
    pub success: Option<Rgba8>,
    /// Defaults to the accent.
    #[serde(default)]
    pub info: Option<Rgba8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Dark,
    Light,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    #[error("{file}: {message}")]
    Parse { file: String, message: String },
}

/// Parses one theme. The file stem wins over a mismatching `id` (Warning); a newer schema is
/// read best-effort (Warning). A retired key (`RETIRED_METRICS_KEYS`) is an Info each. Out-of-range
/// numbers are clamped.
pub fn parse_theme(src: &str, file_stem: &str) -> Result<(ThemeFile, Vec<ThemeIssue>), ThemeError> {
    let parse_error = |e: toml::de::Error| ThemeError::Parse {
        file: format!("{file_stem}.toml"),
        message: e.message().to_owned(),
    };
    let table: toml::Table = src.parse().map_err(parse_error)?;
    let retired = retired_keys_in(&table);
    let mut file: ThemeFile = table.try_into().map_err(parse_error)?;
    let theme = ThemeId::new(file_stem);
    let mut issues = Vec::new();
    if file.id != file_stem {
        issues.push(ThemeIssue::new(
            &theme,
            IssueLevel::Warning,
            format!(
                "id {:?} differs from the file name; using {file_stem:?}",
                file.id
            ),
        ));
        file_stem.clone_into(&mut file.id);
    }
    if file.schema > THEME_SCHEMA {
        issues.push(ThemeIssue::new(
            &theme,
            IssueLevel::Warning,
            format!("written for a newer PR Monitor (schema {})", file.schema),
        ));
    }
    for message in retired {
        issues.push(ThemeIssue::new(&theme, IssueLevel::Info, message));
    }
    file.seeds.contrast = clamp(file.seeds.contrast, &CONTRAST_RANGE);
    Ok((file, issues))
}

/// serde ignores unknown keys, so retired keys are looked for in the raw table. Returns the
/// issue message of each one found.
fn retired_keys_in(table: &toml::Table) -> Vec<&'static str> {
    let Some(metrics) = table.get("metrics").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    RETIRED_METRICS_KEYS
        .iter()
        .filter(|(key, _)| metrics.contains_key(*key))
        .map(|(_, message)| *message)
        .collect()
}

fn clamp(value: f32, range: &std::ops::RangeInclusive<f32>) -> f32 {
    if value.is_nan() {
        return 0.0_f32.clamp(*range.start(), *range.end());
    }
    value.clamp(*range.start(), *range.end())
}

#[cfg(test)]
mod tests;
