//! Lenient serde helpers for hand-edited settings: an integer outside its type's range
//! saturates instead of failing the whole file, so the config's own clamps can report it.

use serde::{Deserialize, Deserializer};

/// Reads any integer and saturates it into `u8` (negative becomes 0, too large becomes 255).
pub fn saturating_u8<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(deserializer).map(|value| saturate(value, u8::MIN, u8::MAX))
}

/// Reads any integer and saturates it into `u32` (negative becomes 0).
pub fn saturating_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(deserializer).map(|value| saturate(value, u32::MIN, u32::MAX))
}

fn saturate<T: TryFrom<i64>>(value: i64, min: T, max: T) -> T {
    if value < 0 {
        min
    } else {
        T::try_from(value).unwrap_or(max)
    }
}

#[cfg(test)]
mod tests;
