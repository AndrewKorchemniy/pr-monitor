//! Advanced sign-in with the organization's own Entra app registration: authorization code +
//! PKCE through the system browser and a loopback redirect. The client id is always supplied
//! by the user; Microsoft first-party ids are never pre-filled.

pub mod loopback;
pub mod pkce;
pub mod token;
pub mod urls;

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, header};
use prm_ado::{HttpTransport, http::HttpRequest};
use prm_domain::{
    auth::{AccessToken, AuthError, AuthHeader, AuthMethod},
    person::non_blank,
    ports::{Clock, UrlOpener},
};
use tokio_util::sync::CancellationToken;

use crate::{
    consts::{ENTRA_FALLBACK_LIFETIME, ENTRA_SCOPES, ENTRA_SIGNIN_TIMEOUT},
    entra::{
        loopback::Loopback,
        pkce::{Pkce, new_state},
        token::{TokenResponse, parse_token_response},
        urls::{AuthorizeRequest, authorize_url, token_url},
    },
    secrets::{SecretKey, SecretStore},
    source::{AuthContext, TokenSource, network},
};

/// The tenant to use when neither the config nor discovery names one.
const FALLBACK_TENANT: &str = "organizations";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntraConfig {
    pub client_id: uuid::Uuid,
    /// GUID, domain or `organizations`; `None` means the org's discovered tenant.
    pub tenant: Option<String>,
}

pub struct EntraSource {
    config: Option<EntraConfig>,
    transport: Arc<dyn HttpTransport>,
    secrets: Arc<dyn SecretStore>,
    opener: Arc<dyn UrlOpener>,
    clock: Arc<dyn Clock>,
    key: SecretKey,
}

/// Everything `EntraSource` talks to.
pub struct EntraDeps {
    pub transport: Arc<dyn HttpTransport>,
    pub secrets: Arc<dyn SecretStore>,
    pub opener: Arc<dyn UrlOpener>,
    pub clock: Arc<dyn Clock>,
}

impl EntraSource {
    pub fn new(config: Option<EntraConfig>, deps: EntraDeps, key: SecretKey) -> Self {
        Self {
            config,
            transport: deps.transport,
            secrets: deps.secrets,
            opener: deps.opener,
            clock: deps.clock,
            key,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn has_refresh_token(&self) -> bool {
        matches!(self.secrets.get(&self.key), Ok(Some(_)))
    }

    pub fn forget(&self) -> Result<(), AuthError> {
        Ok(self.secrets.delete(&self.key)?)
    }

    /// Opens the browser, waits for the redirect (5 minutes), exchanges the code, stores the
    /// refresh token and returns the access token.
    pub async fn sign_in_interactive(
        &self,
        ctx: &AuthContext,
        cancel: CancellationToken,
    ) -> Result<AccessToken, AuthError> {
        let config = self.config.as_ref().ok_or(AuthError::EntraNotConfigured)?;
        let tenant = tenant_for(config, ctx)?;
        let loopback = Loopback::bind().await.map_err(|e| {
            AuthError::Other(format!("Couldn't start the sign-in listener: {e}").into())
        })?;
        let pkce = Pkce::generate()?;
        let state = new_state()?;
        let redirect_uri = loopback.redirect_uri();
        let request = AuthorizeRequest {
            tenant: &tenant,
            redirect_uri: &redirect_uri,
            pkce: &pkce,
            state: &state,
        };
        self.opener
            .open(&authorize_url(config, &request))
            .map_err(|e| AuthError::Other(format!("Couldn't open the browser: {e}").into()))?;
        let code = loopback
            .wait_for_code(&state, cancel, ENTRA_SIGNIN_TIMEOUT)
            .await?;
        let client_id = config.client_id.to_string();
        let form = [
            ("client_id", client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", pkce.verifier.as_str()),
            ("scope", ENTRA_SCOPES),
        ];
        self.redeem(&tenant, &form).await
    }

    /// POSTs a token request, stores any new refresh token (always rotate), returns the token.
    /// The form is borrowed so secrets are copied only into the request body.
    async fn redeem(&self, tenant: &str, form: &[(&str, &str)]) -> Result<AccessToken, AuthError> {
        let body: String = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form.iter().copied())
            .finish();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        let request = HttpRequest {
            method: Method::POST,
            url: token_url(tenant),
            headers,
            body: Some(Bytes::from(body)),
        };
        let response = self.transport.send(request).await.map_err(network)?;
        let tokens = parse_token_response(response.status.as_u16(), &response.body)?;
        self.store_refresh(&tokens)?;
        Ok(self.access_token(tokens))
    }

    fn store_refresh(&self, tokens: &TokenResponse) -> Result<(), AuthError> {
        let Some(refresh) = &tokens.refresh_token else {
            return Ok(());
        };
        Ok(self.secrets.set(&self.key, refresh.expose().as_bytes())?)
    }

    fn access_token(&self, tokens: TokenResponse) -> AccessToken {
        // A missing or zero `expires_in` would make the token stale on arrival.
        let lifetime = match tokens.expires_in {
            0 => ENTRA_FALLBACK_LIFETIME,
            secs => Duration::from_secs(secs),
        };
        let expires_at = self.clock.now().saturating_add(lifetime);
        AccessToken {
            id: tokens.access_token.id(),
            header: AuthHeader::Bearer(tokens.access_token),
            expires_at: Some(expires_at),
            method: AuthMethod::Entra,
        }
    }
}

/// Config tenant, else the discovered tenant, else `organizations`. MSA orgs can't use Entra.
fn tenant_for(config: &EntraConfig, ctx: &AuthContext) -> Result<String, AuthError> {
    if ctx.tenant.is_some_and(|t| t.msa) {
        return Err(AuthError::MsaOrgUnsupported);
    }
    let configured = config.tenant.clone().filter(|t| non_blank(t));
    let discovered = ctx.tenant.map(|t| t.id.to_string());
    Ok(configured
        .or(discovered)
        .unwrap_or_else(|| FALLBACK_TENANT.to_owned()))
}

#[async_trait::async_trait]
impl TokenSource for EntraSource {
    fn method(&self) -> AuthMethod {
        AuthMethod::Entra
    }

    /// Silent refresh with the stored refresh token. Never opens a browser.
    async fn acquire(&self, ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        let config = self.config.as_ref().ok_or(AuthError::EntraNotConfigured)?;
        let tenant = tenant_for(config, ctx)?;
        let stored =
            self.secrets
                .get(&self.key)?
                .ok_or_else(|| AuthError::InteractionRequired {
                    reason: "No saved sign-in for your organization's app.".into(),
                })?;
        // Borrowed from the zeroizing `SecretBytes`: no plain copy of the refresh token.
        let refresh = std::str::from_utf8(&stored).map_err(|_| AuthError::InteractionRequired {
            reason: "The saved sign-in for your organization's app is unreadable.".into(),
        })?;
        let client_id = config.client_id.to_string();
        let form = [
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("scope", ENTRA_SCOPES),
        ];
        self.redeem(&tenant, &form).await
    }
}

#[cfg(test)]
mod tests;
