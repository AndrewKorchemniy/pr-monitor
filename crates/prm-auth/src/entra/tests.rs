use std::sync::Arc;

use prm_ado::testing::{FakeTransport, Scripted};
use prm_domain::testing::{ManualClock, RecordingUrlOpener, org, ts};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{loopback::parse_request_line, token::map_oauth_error, *};
use crate::secrets::InMemorySecretStore;

const TOKEN_OK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/entra_token_ok.json"
));
const INVALID_GRANT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/entra_error_invalid_grant.json"
));
const CONSENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/entra_error_consent.json"
));

fn config() -> EntraConfig {
    EntraConfig {
        client_id: uuid::Uuid::from_u128(0xc1),
        tenant: None,
    }
}

#[test]
fn pkce_rfc7636_vector() {
    let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
    assert_eq!(
        pkce.challenge,
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(Pkce::generate().unwrap().verifier.len(), 43);
}

#[test]
fn authorize_url_snapshot() {
    let pkce = Pkce::from_verifier("verifier".into());
    let request = AuthorizeRequest {
        tenant: "organizations",
        redirect_uri: "http://localhost:51234",
        pkce: &pkce,
        state: "state-1",
    };
    insta::assert_snapshot!(authorize_url(&config(), &request).as_str());
}

#[test]
fn parse_request_line_code_state() {
    let params = parse_request_line("GET /?code=abc&state=s1 HTTP/1.1").unwrap();
    assert_eq!(params.get("code").map(String::as_str), Some("abc"));
    assert_eq!(params.get("state").map(String::as_str), Some("s1"));
    assert_eq!(parse_request_line("GET /favicon.ico HTTP/1.1"), None);
}

#[test]
fn parse_request_line_error() {
    let params =
        parse_request_line("GET /?error=access_denied&error_description=nope HTTP/1.1").unwrap();
    assert_eq!(
        params.get("error").map(String::as_str),
        Some("access_denied")
    );
}

#[test]
fn token_error_mapping() {
    assert!(matches!(
        map_oauth_error("invalid_grant", "x", &[]),
        AuthError::InteractionRequired { .. }
    ));
    assert!(matches!(
        map_oauth_error("interaction_required", "", &[]),
        AuthError::InteractionRequired { .. }
    ));
    assert!(matches!(
        map_oauth_error("invalid_grant", "", &[65_001]),
        AuthError::ConsentRequired { .. }
    ));
    assert!(matches!(
        map_oauth_error("x", "AADSTS53003: blocked", &[]),
        AuthError::ConditionalAccessBlocked(_)
    ));
    let parsed = parse_token_response(400, INVALID_GRANT.as_bytes());
    assert!(matches!(parsed, Err(AuthError::InteractionRequired { .. })));
    let parsed = parse_token_response(400, CONSENT.as_bytes());
    assert!(matches!(parsed, Err(AuthError::ConsentRequired { .. })));
}

/// Declining consent at the account picker came back as a transient `Other` with a Retry
/// action instead of a cancel.
#[test]
fn token_error_user_declined_is_cancelled() {
    assert_eq!(
        map_oauth_error(
            "access_denied",
            "AADSTS65004: User declined to consent.",
            &[]
        ),
        AuthError::Cancelled
    );
    assert_eq!(
        map_oauth_error("access_denied", "", &[]),
        AuthError::Cancelled
    );
    assert!(matches!(
        map_oauth_error("access_denied", "AADSTS53003: blocked", &[]),
        AuthError::ConditionalAccessBlocked(_)
    ));
}

/// A gateway's HTML error page used to read "Sign-in failed ()." with nothing in the brackets.
#[test]
fn token_non_json_error_names_the_status() {
    let parsed = parse_token_response(502, b"<html>Bad Gateway</html>");
    assert_eq!(
        parsed.err(),
        Some(AuthError::Other("Sign-in failed (HTTP 502).".into()))
    );
}

/// A response without `expires_in` used to yield a token that was already stale.
#[tokio::test]
async fn refresh_without_expires_in_uses_fallback_lifetime() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let key = SecretKey::entra_refresh(&org("contoso"));
    secrets.set(&key, b"refresh-1").unwrap();
    let transport = Arc::new(FakeTransport::new([Scripted::json(
        200,
        r#"{"access_token":"at"}"#,
    )]));
    let deps = EntraDeps {
        transport,
        secrets,
        opener: Arc::new(RecordingUrlOpener::default()),
        clock: Arc::new(ManualClock::new(ts(1_000))),
    };
    let source = EntraSource::new(Some(config()), deps, key);
    let ctx = AuthContext {
        org: org("contoso"),
        tenant: None,
    };
    let token = source.acquire(&ctx).await.unwrap();
    let fallback = i64::try_from(ENTRA_FALLBACK_LIFETIME.as_secs()).unwrap();
    assert_eq!(token.expires_at, Some(ts(1_000 + fallback)));
}

async fn send_callback(port: u16, path: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn loopback_accepts_code_on_127_0_0_1() {
    let loopback = Loopback::bind().await.unwrap();
    let port = loopback.port();
    let waiter = tokio::spawn(loopback.wait_for_code(
        "s1",
        CancellationToken::new(),
        Duration::from_secs(10),
    ));
    let favicon = send_callback(port, "/favicon.ico").await;
    assert!(favicon.starts_with("HTTP/1.1 404"));
    let reply = send_callback(port, "/?code=the-code&state=s1").await;
    assert!(reply.starts_with("HTTP/1.1 200"));
    assert_eq!(waiter.await.unwrap(), Ok("the-code".to_owned()));
}

/// A request with the wrong `state` (or an `error` from anyone) used to abort the sign-in; it
/// is now answered 400 and the real callback that follows still completes it.
#[tokio::test]
async fn loopback_state_mismatch_ignored() {
    let loopback = Loopback::bind().await.unwrap();
    let port = loopback.port();
    let waiter = tokio::spawn(loopback.wait_for_code(
        "expected",
        CancellationToken::new(),
        Duration::from_secs(10),
    ));
    let forged = send_callback(port, "/?code=x&state=forged").await;
    assert!(forged.starts_with("HTTP/1.1 400"), "{forged}");
    let error = send_callback(port, "/?error=interaction_required").await;
    assert!(error.starts_with("HTTP/1.1 400"), "{error}");
    let reply = send_callback(port, "/?code=real&state=expected").await;
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert_eq!(waiter.await.unwrap(), Ok("real".to_owned()));
}

/// A silent connection (a browser preconnect) used to block the real callback until the
/// browser closed it.
#[tokio::test]
async fn loopback_idle_connection_does_not_block_callback() {
    let loopback = Loopback::bind().await.unwrap();
    let port = loopback.port();
    let waiter = tokio::spawn(loopback.wait_for_code(
        "s1",
        CancellationToken::new(),
        Duration::from_secs(10),
    ));
    let _idle = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let reply = send_callback(port, "/?code=the-code&state=s1").await;
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert_eq!(waiter.await.unwrap(), Ok("the-code".to_owned()));
}

/// Cancel used to go unnoticed while a silent connection was open.
#[tokio::test]
async fn loopback_cancel_with_idle_connection() {
    let loopback = Loopback::bind().await.unwrap();
    let port = loopback.port();
    let cancel = CancellationToken::new();
    let waiter =
        tokio::spawn(loopback.wait_for_code("s1", cancel.clone(), Duration::from_secs(10)));
    let mut idle = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    // Part of a request, so the handler is mid-read when cancel arrives.
    idle.write_all(b"GET /?code=x").await.unwrap();
    cancel.cancel();
    assert_eq!(waiter.await.unwrap(), Err(AuthError::Cancelled));
}

#[tokio::test]
async fn refresh_replaces_stored_refresh_token() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let key = SecretKey::entra_refresh(&org("contoso"));
    secrets.set(&key, b"refresh-1").unwrap();
    let transport = Arc::new(FakeTransport::new([Scripted::json(200, TOKEN_OK)]));
    let deps = EntraDeps {
        transport: transport.clone(),
        secrets: secrets.clone(),
        opener: Arc::new(RecordingUrlOpener::default()),
        clock: Arc::new(ManualClock::new(ts(1_000))),
    };
    let source = EntraSource::new(Some(config()), deps, key.clone());
    let ctx = AuthContext {
        org: org("contoso"),
        tenant: None,
    };
    let token = source.acquire(&ctx).await.unwrap();
    assert_eq!(token.method, AuthMethod::Entra);
    assert_eq!(token.expires_at, Some(ts(1_000 + 3_599)));
    assert_eq!(
        secrets.get(&key).unwrap().map(|v| v.to_vec()),
        Some(b"refresh-2".to_vec())
    );
    let body = transport.requests()[0].body.clone().unwrap();
    let form = String::from_utf8(body.to_vec()).unwrap();
    assert!(form.contains("grant_type=refresh_token") && form.contains("refresh_token=refresh-1"));
    assert!(!form.contains("client_secret"));
}

#[tokio::test]
async fn entra_not_configured_fails_fast() {
    let deps = EntraDeps {
        transport: Arc::new(FakeTransport::default()),
        secrets: Arc::new(InMemorySecretStore::default()),
        opener: Arc::new(RecordingUrlOpener::default()),
        clock: Arc::new(ManualClock::new(ts(0))),
    };
    let source = EntraSource::new(None, deps, SecretKey::entra_refresh(&org("contoso")));
    let ctx = AuthContext {
        org: org("contoso"),
        tenant: None,
    };
    assert_eq!(
        source.acquire(&ctx).await,
        Err(AuthError::EntraNotConfigured)
    );
}
