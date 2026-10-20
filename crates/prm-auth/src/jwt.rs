//! Reads the `exp` claim of a JWT without verifying it (only used to schedule refresh).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

pub fn jwt_exp(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_i64()
}

#[cfg(test)]
mod tests;
