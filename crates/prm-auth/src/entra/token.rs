//! Parsing Entra token responses and mapping OAuth errors to `AuthError`.

use prm_domain::{Secret, auth::AuthError};
use serde::Deserialize;

/// A successful token response. Tokens are `Secret`s from the moment they are parsed.
#[derive(Debug)]
pub struct TokenResponse {
    pub access_token: Secret,
    pub expires_in: u64,
    pub refresh_token: Option<Secret>,
}

#[derive(Deserialize)]
struct SuccessBody {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
    #[serde(default)]
    error_codes: Vec<i64>,
}

pub fn parse_token_response(status: u16, body: &[u8]) -> Result<TokenResponse, AuthError> {
    if (200..300).contains(&status)
        && let Ok(ok) = serde_json::from_slice::<SuccessBody>(body)
    {
        return Ok(TokenResponse {
            access_token: Secret::new(ok.access_token),
            expires_in: ok.expires_in,
            refresh_token: ok.refresh_token.filter(|t| !t.is_empty()).map(Secret::new),
        });
    }
    match serde_json::from_slice::<ErrorBody>(body) {
        Ok(error) if !error.error.is_empty() => Err(map_oauth_error(
            &error.error,
            &error.error_description,
            &error.error_codes,
        )),
        // A gateway error page or other non-OAuth body: the status is all there is.
        Ok(_) | Err(_) => Err(AuthError::Other(
            format!("Sign-in failed (HTTP {status}).").into(),
        )),
    }
}

/// `invalid_grant` / `interaction_required` → sign in again; consent and Conditional Access
/// codes → admin; the user declining (`access_denied`, AADSTS65004) → cancelled; anything else
/// → the description's first line.
pub fn map_oauth_error(error: &str, description: &str, codes: &[i64]) -> AuthError {
    let first_line = description.lines().next().unwrap_or_default().trim();
    let has_code =
        |code: i64| codes.contains(&code) || description.contains(&format!("AADSTS{code}"));
    if has_code(53_003) {
        AuthError::ConditionalAccessBlocked(first_line.into())
    } else if has_code(65_001) || has_code(90_094) {
        AuthError::ConsentRequired {
            admin: true,
            detail: first_line.into(),
        }
    } else if has_code(65_004) || error == "access_denied" {
        AuthError::Cancelled
    } else if error == "invalid_grant" || error == "interaction_required" {
        AuthError::InteractionRequired {
            reason: first_line.into(),
        }
    } else if first_line.is_empty() {
        AuthError::Other(format!("Sign-in failed ({error}).").into())
    } else {
        AuthError::Other(first_line.into())
    }
}
