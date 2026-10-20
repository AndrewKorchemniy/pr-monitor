use prm_ado::testing::token;

use super::*;
use crate::testing::FakeTokenSource;

fn ctx() -> AuthContext {
    AuthContext {
        org: prm_domain::testing::org("contoso"),
        tenant: None,
    }
}

fn source(method: AuthMethod, result: Result<AccessToken, AuthError>) -> Arc<FakeTokenSource> {
    Arc::new(FakeTokenSource::new(method, [result]))
}

#[tokio::test]
async fn chain_prefers_last_good() {
    let az = source(AuthMethod::AzureCli, Ok(token("az")));
    let pat = source(AuthMethod::Pat, Ok(token("pat")));
    let chain = AutoChain::new(vec![az.clone(), pat.clone()], Some(AuthMethod::Pat));
    let got = chain.acquire(&ctx()).await.unwrap();
    assert_eq!(got.id, token("pat").id);
    assert_eq!(az.acquired(), 0);
}

#[tokio::test]
async fn chain_falls_through_and_remembers() {
    let az = source(AuthMethod::AzureCli, Err(AuthError::AzNotInstalled));
    let pat = source(AuthMethod::Pat, Ok(token("pat")));
    let chain = AutoChain::new(vec![az, pat], None);
    chain.acquire(&ctx()).await.unwrap();
    assert_eq!(chain.preferred(), Some(AuthMethod::Pat));
}

#[tokio::test]
async fn chain_most_actionable() {
    let az = source(
        AuthMethod::AzureCli,
        Err(AuthError::AzNotLoggedIn { tenant: None }),
    );
    let pat = source(AuthMethod::Pat, Err(AuthError::PatMissing));
    let chain = AutoChain::new(vec![az, pat], None);
    assert_eq!(
        chain.acquire(&ctx()).await,
        Err(AuthError::AzNotLoggedIn { tenant: None })
    );
}

#[tokio::test]
async fn chain_hard_stop_conditional_access() {
    let az = source(
        AuthMethod::AzureCli,
        Err(AuthError::ConditionalAccessBlocked("x".into())),
    );
    let pat = source(AuthMethod::Pat, Ok(token("pat")));
    let chain = AutoChain::new(vec![az, pat.clone()], None);
    assert!(matches!(
        chain.acquire(&ctx()).await,
        Err(AuthError::ConditionalAccessBlocked(_))
    ));
    assert_eq!(pat.acquired(), 0);
}

#[tokio::test]
async fn chain_all_unconfigured_no_method() {
    let chain = AutoChain::new(
        vec![
            source(AuthMethod::AzureCli, Err(AuthError::AzNotInstalled)),
            source(AuthMethod::Entra, Err(AuthError::EntraNotConfigured)),
            source(AuthMethod::Pat, Err(AuthError::PatMissing)),
        ],
        None,
    );
    let Err(AuthError::NoMethod { tried }) = chain.acquire(&ctx()).await else {
        panic!("expected NoMethod");
    };
    assert_eq!(tried.len(), 3);
}
