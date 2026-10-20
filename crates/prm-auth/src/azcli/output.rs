//! Parsing `az account get-access-token` output.

use prm_domain::{Secret, Timestamp, auth::AuthError};
use serde::Deserialize;

use crate::{consts::AZ_FALLBACK_LIFETIME, jwt::jwt_exp};

#[derive(Deserialize)]
struct AzTokenOut {
    #[serde(rename = "accessToken", default)]
    access_token: String,
    /// POSIX seconds (az >= 2.54). `expiresOn` is local time without a zone and is ignored.
    #[serde(rename = "expires_on", default)]
    expires_on: Option<NumberOrString>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NumberOrString {
    Number(i64),
    Text(String),
}

impl NumberOrString {
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(s) => s.trim().parse().ok(),
        }
    }
}

/// The token and when it expires: `expires_on`, else the JWT `exp`, else now + 5 minutes.
pub fn parse_token_output(stdout: &[u8], now: Timestamp) -> Result<(Secret, Timestamp), AuthError> {
    let out: AzTokenOut = serde_json::from_slice(stdout).map_err(|e| AuthError::AzFailed {
        stderr: format!("unreadable az output: {e}").into(),
        exit_code: Some(0),
    })?;
    let token = out.access_token.trim();
    if token.is_empty() {
        return Err(AuthError::AzFailed {
            stderr: "az returned an empty token".into(),
            exit_code: Some(0),
        });
    }
    let expires_at = out
        .expires_on
        .as_ref()
        .and_then(NumberOrString::as_i64)
        .or_else(|| jwt_exp(token))
        .and_then(Timestamp::from_unix)
        .unwrap_or_else(|| {
            tracing::warn!("az token has no expiry; assuming 5 minutes");
            now.saturating_add(AZ_FALLBACK_LIFETIME)
        });
    Ok((Secret::new(token), expires_at))
}
