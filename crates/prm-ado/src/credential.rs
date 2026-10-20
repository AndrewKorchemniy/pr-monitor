//! Where the client gets its `Authorization` header. prm-auth implements it per org.

use http::HeaderValue;
use prm_domain::{
    auth::{AccessToken, AuthError, AuthHeader},
    secret::TokenId,
};

#[async_trait::async_trait]
pub trait Credential: Send + Sync + 'static {
    /// `rejected = Some(id)`: the caller got a 401 with that token. Refresh only if the cached
    /// token is still `id`, so concurrent 401s refresh once.
    async fn access_token(&self, rejected: Option<TokenId>) -> Result<AccessToken, AuthError>;
}

/// A fixed token, e.g. a PAT being validated before it is stored.
#[derive(Clone, Debug)]
pub struct StaticCredential(pub AccessToken);

#[async_trait::async_trait]
impl Credential for StaticCredential {
    async fn access_token(&self, rejected: Option<TokenId>) -> Result<AccessToken, AuthError> {
        match rejected {
            Some(id) if id == self.0.id => Err(AuthError::Unauthorized { code: None }),
            Some(_) | None => Ok(self.0.clone()),
        }
    }
}

/// `Bearer …` / `Basic …`, marked sensitive so it never prints.
pub fn auth_header_value(header: &AuthHeader) -> Result<HeaderValue, AuthError> {
    let mut value = HeaderValue::from_str(&header.header_value())
        .map_err(|_| AuthError::Other("The token contains invalid characters.".into()))?;
    value.set_sensitive(true);
    Ok(value)
}
