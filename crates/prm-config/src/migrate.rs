//! Config schema migrations, applied to the raw TOML table before deserializing.

use std::sync::Arc;

use crate::error::ConfigError;

pub const CURRENT_SCHEMA: u32 = 1;

/// Upgrades a table from version N to N + 1 in place.
pub type Migration = fn(&mut toml::Table) -> Result<(), String>;

/// `(from_version, migration)`. v1 is the first release; append `(1, v1_to_v2)` when needed.
pub const MIGRATIONS: &[(u32, Migration)] = &[];

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
