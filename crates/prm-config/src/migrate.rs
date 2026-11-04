//! Config schema migrations, applied to the raw TOML table before deserializing.

use std::sync::Arc;

use prm_theme::{Corners, registry};

use crate::error::ConfigError;

pub const CURRENT_SCHEMA: u32 = 1;

/// Upgrades a table from version N to N + 1 in place.
pub type Migration = fn(&mut toml::Table) -> Result<(), String>;

/// `(from_version, migration)`. v1 is the first release; append `(1, v1_to_v2)` when needed.
pub const MIGRATIONS: &[(u32, Migration)] = &[];

/// The one theme that was square before corners became a setting.
const SQUARE_THEME: &str = "cyanotype";

/// The corners a config written before `ui.corners` existed should get, so its look does not
/// change: Square when its theme was Cyanotype (picked directly, or as the dark or light theme
/// while following Windows), else Rounded. `None` when the config already has `ui.corners`.
///
/// This is an unversioned, idempotent step that runs on every load: adding a key needs no
/// schema bump, and older builds ignore the key. Once a save writes it, this is a no-op.
pub fn legacy_corners(table: &toml::Table) -> Option<Corners> {
    let ui = table.get("ui").and_then(toml::Value::as_table);
    let text = |key: &str| ui.and_then(|ui| ui.get(key)).and_then(toml::Value::as_str);
    if ui.is_some_and(|ui| ui.contains_key("corners")) {
        return None;
    }
    let is_square_theme = |key: &str, default: &str| text(key).unwrap_or(default) == SQUARE_THEME;
    let square = match text("theme") {
        Some(registry::SYSTEM) | None => {
            is_square_theme("theme_dark", registry::DEFAULT_DARK)
                || is_square_theme("theme_light", registry::DEFAULT_LIGHT)
        }
        Some(picked) => picked == SQUARE_THEME,
    };
    Some(if square {
        Corners::Square
    } else {
        Corners::Rounded
    })
}

/// Runs migrations `from → to` in order and stamps `schema_version`. Returns the new version.
pub fn migrate_with(
    table: &mut toml::Table,
    from: u32,
    to: u32,
    migrations: &[(u32, Migration)],
) -> Result<u32, ConfigError> {
    let mut version = from;
    while version < to {
        let (_, step) = migrations
            .iter()
            .find(|(v, _)| *v == version)
            .ok_or_else(|| failure(version, "no migration registered"))?;
        step(table).map_err(|message| failure(version, &message))?;
        version += 1;
        table.insert(
            "schema_version".into(),
            toml::Value::Integer(i64::from(version)),
        );
    }
    Ok(version)
}

fn failure(from: u32, message: &str) -> ConfigError {
    ConfigError::Migration {
        from,
        to: from + 1,
        message: Arc::from(message),
    }
}

#[cfg(test)]
mod tests;
