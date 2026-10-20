use prm_ado::testing::{FakeTransport, Scripted};
use prm_domain::{
    auth::TenantInfo,
    testing::{ManualClock, RecordingUrlOpener, org, ts},
};

use super::*;
use crate::{
    azcli::locate::AzLocator,
    consts::{NEG_CACHE_TTL, TOKEN_SKEW},
    entra::EntraDeps,
    secrets::{InMemorySecretStore, SecretKey, SecretStore},
    testing::{FakeCommandRunner, success},
};

const TOKEN_OK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/az_token_ok.json"
));
const TENANT: uuid::Uuid = uuid::Uuid::from_u128(0xd1);

fn pat() -> Secret {
    let mut value = "a".repeat(76);
    value.push_str("AZDOxxxx");
    Secret::new(value)
}

struct Fixture {
    auth: OrgAuth,
    runner: Arc<FakeCommandRunner>,
    secrets: Arc<InMemorySecretStore>,
}

fn fixture(mode: AuthMode, signed_out: bool, override_tenant: bool) -> Fixture {
    let runner = Arc::new(FakeCommandRunner::default().answer("account", success(TOKEN_OK)));
    let secrets = Arc::new(InMemorySecretStore::default());
    let clock: Arc<ManualClock> = Arc::new(ManualClock::new(ts(1_000)));
    let transport = Arc::new(FakeTransport::new([Scripted::json(401, "").header(
        "x-vss-resourcetenant",
        "00000000-0000-0000-0000-0000000000aa",
    )]));
    let spec = AuthSpec {
        org: org("contoso"),
        endpoint: OrgEndpoint::new(org("contoso")),
        mode,
        tenant_override: override_tenant.then_some(TENANT),
        tenant_cached: None,
        entra: None,
        preferred: None,
        signed_out,
    };
    let sources = Sources {
        az: Arc::new(AzureCliSource::new(
            runner.clone(),
            AzLocator::fixed("az.cmd".into()),
            clock.clone(),
        )),
        pat: Arc::new(PatSource::new(secrets.clone(), SecretKey::pat(&spec.org))),
        entra: Arc::new(EntraSource::new(
            None,
            EntraDeps {
                transport: transport.clone(),
                secrets: secrets.clone(),
                opener: Arc::new(RecordingUrlOpener::default()),
                clock: clock.clone(),
            },
            SecretKey::entra_refresh(&spec.org),
        )),
    };
    let cache = TokenCache::new(clock.clone(), TOKEN_SKEW, NEG_CACHE_TTL);
    let auth = OrgAuth::new(spec, sources, cache, TenantDiscovery::new(transport), clock);
    Fixture {
        auth,
        runner,
        secrets,
    }
}

#[tokio::test]
async fn org_auth_signed_out_blocks_silent() {
    let f = fixture(AuthMode::Auto, true, true);
    assert_eq!(f.auth.access_token(None).await, Err(AuthError::SignedOut));
    assert_eq!(f.runner.specs().len(), 0);
}

#[tokio::test]
async fn org_auth_store_pat_prefers_pat_and_invalidates() {
    let f = fixture(AuthMode::Auto, true, true);
    f.auth.store_pat(pat()).await.unwrap();
    let token = f.auth.access_token(None).await.unwrap();
    assert_eq!(token.method, AuthMethod::Pat);
    assert!(f.runner.specs().is_empty(), "PAT preferred over az");
    assert!(!f.auth.snapshot().signed_out);
}

#[tokio::test]
async fn org_auth_tenant_override_wins() {
    let f = fixture(AuthMode::AzureCli, false, true);
    f.auth.access_token(None).await.unwrap();
    assert_eq!(f.auth.snapshot().tenant, Some(TenantInfo::from_id(TENANT)));
    let args: Vec<String> = f.runner.specs()[0]
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&TENANT.to_string()));
}

#[tokio::test]
async fn org_auth_discovers_tenant_without_override() {
    let f = fixture(AuthMode::AzureCli, false, false);
    f.auth.access_token(None).await.unwrap();
    assert_eq!(
        f.auth.snapshot().tenant.map(|t| t.id),
        Some(uuid::Uuid::from_u128(0xaa))
    );
}

#[tokio::test]
async fn org_auth_sign_out_deletes_secrets() {
    let f = fixture(AuthMode::Auto, false, true);
    f.auth.store_pat(pat()).await.unwrap();
    f.auth.sign_out().await.unwrap();
    assert_eq!(f.secrets.keys(), Vec::<String>::new());
    assert_eq!(f.auth.access_token(None).await, Err(AuthError::SignedOut));
}

#[tokio::test]
async fn org_auth_az_login_then_token() {
    let f = fixture(AuthMode::Auto, true, true);
    let runner = Arc::new(
        FakeCommandRunner::default()
            .answer("login", success(""))
            .answer("account", success(TOKEN_OK)),
    );
    // Re-wire with a runner that also answers `login`.
    let f = Fixture {
        auth: OrgAuth::new(
            f.auth.spec.clone(),
            Sources {
                az: Arc::new(AzureCliSource::new(
                    runner.clone(),
                    AzLocator::fixed("az.cmd".into()),
                    Arc::new(ManualClock::new(ts(1_000))),
                )),
                pat: f.auth.sources.pat.clone(),
                entra: f.auth.sources.entra.clone(),
            },
            TokenCache::new(
                Arc::new(ManualClock::new(ts(1_000))),
                TOKEN_SKEW,
                NEG_CACHE_TTL,
            ),
            TenantDiscovery::new(Arc::new(FakeTransport::default())),
            Arc::new(ManualClock::new(ts(1_000))),
        ),
        runner,
        secrets: f.secrets,
    };
    let method = f
        .auth
        .sign_in(
            SignInMethod::AzLogin {
                broker: prm_domain::auth::AzBroker::Default,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(method, AuthMethod::AzureCli);
    assert_eq!(f.runner.specs().len(), 2);
    assert!(
        f.secrets
            .get(&SecretKey::pat(&org("contoso")))
            .unwrap()
            .is_none()
    );
}
