//! Personal access tokens: stored per org, sent as `Basic base64(":" + pat)`.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use prm_domain::{
    Secret,
    auth::{AccessToken, AuthError, AuthHeader, AuthMethod},
};

use crate::{
    secrets::{SecretKey, SecretStore},
    source::{AuthContext, TokenSource},
};

/// Legacy PAT length (52 base32-ish chars).
const LEGACY_PAT_LEN: usize = 52;
/// Current PAT length; characters 76..80 are the `AZDO` signature.
const PAT_LEN: usize = 84;
const PAT_SIGNATURE: std::ops::Range<usize> = 76..80;

/// Accepts the 52-char legacy format and the 84-char format with the `AZDO` signature.
pub fn validate_pat_format(pat: &str) -> Result<(), AuthError> {
    let pat = pat.trim();
    let alphanumeric = pat.bytes().all(|b| b.is_ascii_alphanumeric());
    let valid = alphanumeric
        && (pat.len() == LEGACY_PAT_LEN
            || (pat.len() == PAT_LEN && pat.get(PAT_SIGNATURE) == Some("AZDO")));
    if valid {
        Ok(())
    } else {
        Err(AuthError::PatFormat)
    }
}

/// `Basic base64(":" + pat)`: an empty username, which is what Azure DevOps expects.
pub fn basic_header(pat: &Secret) -> AuthHeader {
    let encoded = STANDARD.encode(format!(":{}", pat.expose().trim()));
    AuthHeader::Basic(Secret::new(encoded))
}

pub struct PatSource {
    secrets: Arc<dyn SecretStore>,
    key: SecretKey,
}

impl PatSource {
    pub fn new(secrets: Arc<dyn SecretStore>, key: SecretKey) -> Self {
        Self { secrets, key }
    }

    pub fn store(&self, pat: &Secret) -> Result<(), AuthError> {
        self.secrets
            .set(&self.key, pat.expose().trim().as_bytes())
            .map_err(|e| AuthError::SecretStore(e.to_string().into()))
    }

    pub fn delete(&self) -> Result<(), AuthError> {
        self.secrets
            .delete(&self.key)
            .map_err(|e| AuthError::SecretStore(e.to_string().into()))
    }

    pub fn is_stored(&self) -> bool {
        matches!(self.secrets.get(&self.key), Ok(Some(_)))
    }
}

#[async_trait::async_trait]
impl TokenSource for PatSource {
    fn method(&self) -> AuthMethod {
        AuthMethod::Pat
    }

    async fn acquire(&self, _ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        let bytes = self
            .secrets
            .get(&self.key)
            .map_err(|e| AuthError::SecretStore(e.to_string().into()))?
            .ok_or(AuthError::PatMissing)?;
        let pat = std::str::from_utf8(&bytes).map_err(|_| AuthError::PatFormat)?;
        let header = basic_header(&Secret::new(pat));
        Ok(AccessToken {
            id: header.secret().id(),
            header,
            expires_at: None,
            method: AuthMethod::Pat,
        })
    }
}

#[cfg(test)]
mod tests;
