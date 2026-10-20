//! Entra endpoints for the authorization-code flow.

use url::Url;

use super::{EntraConfig, pkce::Pkce};
use crate::consts::{ENTRA_AUTHORITY, ENTRA_SCOPES};

/// What to put in the authorize URL besides the app config.
pub struct AuthorizeRequest<'a> {
    pub tenant: &'a str,
    pub redirect_uri: &'a str,
    pub pkce: &'a Pkce,
    pub state: &'a str,
    pub login_hint: Option<&'a str>,
}

pub fn authorize_url(cfg: &EntraConfig, request: &AuthorizeRequest<'_>) -> Url {
    let mut url = endpoint(request.tenant, "authorize");
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("client_id", &cfg.client_id.to_string())
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", request.redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", ENTRA_SCOPES)
            .append_pair("state", request.state)
            .append_pair("code_challenge", &request.pkce.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("prompt", "select_account");
        if let Some(hint) = request.login_hint {
            query.append_pair("login_hint", hint);
        }
    }
    url
}

pub fn token_url(tenant: &str) -> Url {
    endpoint(tenant, "token")
}

#[expect(clippy::expect_used, reason = "the authority is a constant https URL")]
fn endpoint(tenant: &str, leaf: &str) -> Url {
    let mut url = Url::parse(ENTRA_AUTHORITY).expect("valid authority");
    if let Ok(mut path) = url.path_segments_mut() {
        path.extend([tenant, "oauth2", "v2.0", leaf]);
    }
    url
}
