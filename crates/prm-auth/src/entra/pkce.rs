//! PKCE (RFC 7636, S256) and the anti-forgery `state` value.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use prm_domain::auth::AuthError;
use sha2::{Digest, Sha256};

/// Random bytes in a verifier (encodes to 43 characters).
const VERIFIER_BYTES: usize = 32;
const STATE_BYTES: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// The verifier is a one-time secret: redacted. The challenge is public (it is in the URL).
impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce")
            .field("verifier", &"<redacted>")
            .field("challenge", &self.challenge)
            .finish()
    }
}

impl Pkce {
    pub fn generate() -> Result<Self, AuthError> {
        Ok(Self::from_verifier(random_b64url(VERIFIER_BYTES)?))
    }

    /// `challenge = base64url(sha256(verifier))`.
    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

pub fn new_state() -> Result<String, AuthError> {
    random_b64url(STATE_BYTES)
}

fn random_b64url(len: usize) -> Result<String, AuthError> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes)
        .map_err(|e| AuthError::Other(format!("no randomness available: {e}").into()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
