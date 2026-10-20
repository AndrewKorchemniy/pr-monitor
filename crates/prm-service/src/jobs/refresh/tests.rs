use prm_ado::testing::{ApiCall, FakeAdoApi, FakeOrg};
use prm_auth::testing::FakeOrgAuth;
use prm_domain::testing::{ManualClock, org, ts};

use super::*;

const NOW: i64 = 100_000;

fn ctx(api: Arc<FakeAdoApi>, previous: Option<ProbeCache>) -> RefreshCtx {
    let (progress, _receiver) = tokio::sync::mpsc::unbounded_channel();
    RefreshCtx {
        org: org("contoso"),
        api,
        auth: Arc::new(FakeOrgAuth::default()),
        profile: OrgProfile {
            id: Some(org("contoso")),
            org_wide_query: true,
            ..OrgProfile::default()
        },
        me: None,
        catalog: None,
        clock: Arc::new(ManualClock::new(ts(NOW))),
        prev: Arc::new(BTreeMap::new()),
        org_wide: previous,
        progress,
    }
}

fn catalog() -> Catalog {
    Catalog {
        fetched_at: ts(NOW),
        projects: Vec::new(),
        repos: Vec::new(),
    }
}

fn api() -> Arc<FakeAdoApi> {
    Arc::new(FakeAdoApi::new(
        &org("contoso"),
        FakeOrg {
            org_wide: Some(OrgWideProbe::Supported),
            ..FakeOrg::default()
        },
    ))
}

/// A probe that hit a network error used to be cached as "unsupported" for a day.
#[tokio::test]
async fn org_wide_probe_error_keeps_previous_result() {
    let expired = ProbeCache {
        ok: true,
        at: ts(0),
    };
    for previous in [None, Some(expired)] {
        let api = api();
        api.fail(ApiCall::ProbeOrgWide, AdoError::Timeout);
        let probe = org_wide_probe(&ctx(api.clone(), previous), &catalog()).await;
        assert_eq!(probe, previous, "previous {previous:?}");
        assert_eq!(api.calls(ApiCall::ProbeOrgWide), 1);
    }
}

#[tokio::test]
async fn org_wide_probe_verdict_is_cached() {
    let api = api();
    let probe = org_wide_probe(&ctx(api, None), &catalog()).await;
    assert_eq!(
        probe,
        Some(ProbeCache {
            ok: true,
            at: ts(NOW),
        })
    );
}
