use base64::Engine;

use super::*;
use crate::secrets::InMemorySecretStore;

fn pat_84() -> String {
    let mut pat = "a".repeat(76);
    pat.push_str("AZDO");
    pat.push_str("bbbb");
    pat
}

#[test]
fn pat_basic_header_leading_colon() {
    let AuthHeader::Basic(encoded) = basic_header(&Secret::new(" mypat ")) else {
        panic!("expected Basic");
    };
    let decoded = STANDARD.decode(encoded.expose()).unwrap();
    assert_eq!(decoded, b":mypat");
}

#[test]
fn pat_format_84_azdo_ok() {
    assert_eq!(validate_pat_format(&pat_84()), Ok(()));
}

#[test]
fn pat_format_52_legacy_ok() {
    assert_eq!(validate_pat_format(&"x".repeat(52)), Ok(()));
}

#[test]
fn pat_format_rejects_garbage() {
    for bad in [
        "",
        "short",
        &"a".repeat(84),
        &format!("{}!", "a".repeat(51)),
    ] {
        assert_eq!(validate_pat_format(bad), Err(AuthError::PatFormat), "{bad}");
    }
}

fn ctx() -> AuthContext {
    AuthContext {
        org: prm_domain::testing::org("contoso"),
        tenant: None,
    }
}

#[tokio::test]
async fn pat_missing() {
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let source = PatSource::new(store, SecretKey::pat(&prm_domain::testing::org("contoso")));
    assert_eq!(source.acquire(&ctx()).await, Err(AuthError::PatMissing));
}

#[tokio::test]
async fn pat_stored_becomes_basic_token() {
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let source = PatSource::new(store, SecretKey::pat(&prm_domain::testing::org("contoso")));
    source.store(&Secret::new(pat_84())).unwrap();
    let token = source.acquire(&ctx()).await.unwrap();
    assert_eq!(token.method, AuthMethod::Pat);
    assert!(matches!(token.header, AuthHeader::Basic(_)));
    assert!(source.is_stored());
}
