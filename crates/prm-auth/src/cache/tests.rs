use std::{sync::Arc, time::Duration};

use prm_ado::testing::token;
use prm_domain::{
    auth::AuthMethod,
    testing::{ManualClock, ts},
};

use super::*;
use crate::{
    source::{AuthContext, TokenSource},
    testing::FakeTokenSource,
};

fn ctx() -> AuthContext {
    AuthContext {
        org: prm_domain::testing::org("contoso"),
        tenant: None,
    }
}

fn expiring(value: &str, at: i64) -> AccessToken {
    AccessToken {
        expires_at: Some(ts(at)),
        ..token(value)
    }
}

fn cache(clock: Arc<ManualClock>) -> Arc<TokenCache> {
    Arc::new(TokenCache::new(
        clock,
        Duration::from_secs(120),
        Duration::from_secs(30),
    ))
}

async fn get_owned(
    cache: Arc<TokenCache>,
    source: Arc<FakeTokenSource>,
) -> Result<AccessToken, AuthError> {
    let ctx = ctx();
    cache.get(None, || source.acquire(&ctx)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cache_single_flight_10_concurrent_one_acquire() {
    let source = Arc::new(
        FakeTokenSource::new(AuthMethod::AzureCli, [Ok(token("t"))])
            .with_delay(Duration::from_millis(50)),
    );
    let cache = cache(Arc::new(ManualClock::new(ts(0))));
    let calls = (0..10).map(|_| tokio::spawn(get_owned(cache.clone(), source.clone())));
    for call in calls.collect::<Vec<_>>() {
        call.await.unwrap().unwrap();
    }
    assert_eq!(source.acquired(), 1);
}

#[tokio::test]
async fn cache_skew_refreshes_at_119s_keeps_at_121s() {
    let clock = Arc::new(ManualClock::new(ts(1_000)));
    let cache = cache(clock.clone());
    let ctx = ctx();
    let source = FakeTokenSource::new(AuthMethod::AzureCli, [Ok(expiring("t", 1_121))]);
    cache.get(None, || source.acquire(&ctx)).await.unwrap();
    cache.get(None, || source.acquire(&ctx)).await.unwrap();
    assert_eq!(source.acquired(), 1, "121 s left: keep");

    clock.set(ts(1_002));
    cache.get(None, || source.acquire(&ctx)).await.unwrap();
    assert_eq!(source.acquired(), 2, "119 s left: refresh");
}

#[tokio::test]
async fn cache_rejected_id_refreshes_once_for_concurrent_401s() {
    let cache = cache(Arc::new(ManualClock::new(ts(0))));
    let ctx = ctx();
    let source = FakeTokenSource::new(AuthMethod::AzureCli, [Ok(token("old")), Ok(token("new"))]);
    let old = cache.get(None, || source.acquire(&ctx)).await.unwrap();
    let first = cache
        .get(Some(old.id), || source.acquire(&ctx))
        .await
        .unwrap();
    let second = cache
        .get(Some(old.id), || source.acquire(&ctx))
        .await
        .unwrap();
    assert_eq!(first.id, token("new").id);
    assert_eq!(second.id, token("new").id);
    assert_eq!(source.acquired(), 2);
}

#[tokio::test]
async fn cache_negative_ttl_30s() {
    let clock = Arc::new(ManualClock::new(ts(0)));
    let cache = cache(clock.clone());
    let ctx = ctx();
    let source = FakeTokenSource::new(
        AuthMethod::AzureCli,
        [Err(AuthError::AzTimeout), Ok(token("t"))],
    );
    assert!(cache.get(None, || source.acquire(&ctx)).await.is_err());
    assert!(cache.get(None, || source.acquire(&ctx)).await.is_err());
    assert_eq!(source.acquired(), 1, "failure is reused for 30 s");
    clock.set(ts(31));
    assert!(cache.get(None, || source.acquire(&ctx)).await.is_ok());
}
