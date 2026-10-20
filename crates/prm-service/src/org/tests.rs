use prm_config::schema::{EntraSettings, PeopleConfig};
use prm_domain::{
    diff::ChangeKind,
    testing::{PrBuilder, key, org, person, record, ts},
};

use super::*;

const ORG: &str = "contoso";

fn profile() -> OrgProfile {
    OrgProfile {
        id: Some(org(ORG)),
        url: format!("https://dev.azure.com/{ORG}"),
        people: PeopleConfig {
            tracked_authors: vec![person(2, "Ann")],
            reviewers: vec![person(3, "Rev")],
            ignored_comment_authors: Vec::new(),
        },
        ..OrgProfile::default()
    }
}

fn data() -> OrgData {
    OrgData::new(org(ORG), profile(), None, None, ts(0))
}

fn fetched(numbers: &[i32]) -> Fetched {
    let records = numbers
        .iter()
        .map(|&n| {
            let pr = PrBuilder::new(key(ORG, 1, n))
                .author(person(2, "Ann"))
                .build();
            (pr.key.clone(), record(pr, Vec::new()))
        })
        .collect();
    Fetched {
        me: person(10, "Me"),
        tenant: None,
        catalog: None,
        scope_stats: ScopeStats::default(),
        scope_issues: Vec::new(),
        records,
        stale: BTreeSet::new(),
        closed: Vec::new(),
        harvested: vec![person(2, "Ann")],
        failed_queries: Vec::new(),
        org_wide: None,
    }
}

fn kinds(absorbed: &Absorbed) -> Vec<ChangeKind> {
    absorbed.events.iter().map(|e| e.kind.clone()).collect()
}

#[test]
fn org_absorb_baseline_is_silent() {
    let mut data = data();
    let absorbed = data.absorb(fetched(&[1, 2]), ts(10));
    assert!(absorbed.first_baseline);
    assert_eq!(kinds(&absorbed), Vec::new());
    assert!(data.baseline);
    assert_eq!(data.snapshot.len(), 2);
    assert_eq!(data.last_ok, Some(ts(10)));
}

#[test]
fn org_absorb_reports_new_pr_after_baseline() {
    let mut data = data();
    data.absorb(fetched(&[1]), ts(10));
    let absorbed = data.absorb(fetched(&[1, 2]), ts(20));
    assert!(!absorbed.first_baseline);
    assert_eq!(kinds(&absorbed), vec![ChangeKind::NewPr]);
}

#[test]
fn org_absorb_rebaseline_suppresses_new_pr() {
    let mut data = data();
    data.absorb(fetched(&[1]), ts(10));
    data.rebaseline_pending = true;
    let absorbed = data.absorb(fetched(&[1, 2]), ts(20));
    assert_eq!(kinds(&absorbed), Vec::new());
    assert!(!data.rebaseline_pending);
}

#[test]
fn org_absorb_records_tenant_once() {
    let mut data = data();
    let tenant = TenantInfo::from_id(uuid::Uuid::from_u128(7));
    data.absorb(
        Fetched {
            tenant: Some(tenant),
            ..fetched(&[])
        },
        ts(10),
    );
    data.absorb(
        Fetched {
            tenant: Some(tenant),
            ..fetched(&[])
        },
        ts(20),
    );
    assert_eq!(data.tenant.map(|t| t.discovered_at), Some(ts(10)));
}

const OVERRIDE: &str = "00000000-0000-0000-0000-0000000000d1";

/// The override tenant was saved as a discovered tenant and stayed in use for a week after
/// the user removed a wrong override.
#[test]
fn org_absorb_does_not_save_the_override_tenant() {
    let profile = OrgProfile {
        tenant: OVERRIDE.into(),
        ..profile()
    };
    let mut data = OrgData::new(org(ORG), profile, None, None, ts(0));
    let tenant = TenantInfo::from_id(uuid::Uuid::from_u128(0xd1));
    data.absorb(
        Fetched {
            tenant: Some(tenant),
            ..fetched(&[])
        },
        ts(10),
    );
    assert_eq!(data.tenant, None);
}

#[test]
fn org_set_profile_override_change_forgets_saved_tenant() {
    let mut data = data();
    let tenant = TenantInfo::from_id(uuid::Uuid::from_u128(7));
    data.absorb(
        Fetched {
            tenant: Some(tenant),
            ..fetched(&[])
        },
        ts(10),
    );
    data.set_profile(profile(), ts(20));
    assert!(data.tenant.is_some(), "unchanged override keeps the tenant");
    let with_override = OrgProfile {
        tenant: OVERRIDE.into(),
        ..profile()
    };
    data.set_profile(with_override, ts(30));
    assert_eq!(data.tenant, None);
}

#[test]
fn org_persisted_roundtrip_keeps_baseline_and_records() {
    let mut data = data();
    data.absorb(fetched(&[1]), ts(10));
    let saved = data.to_persisted(Some(AuthMethod::Pat));
    assert_eq!(saved.last_good_method, Some(AuthMethod::Pat));
    let restored = OrgData::new(org(ORG), profile(), Some(saved), None, ts(30));
    assert!(restored.baseline);
    assert_eq!(restored.snapshot.len(), 1);
    assert_eq!(restored.me, Some(person(10, "Me")));
}

#[test]
fn org_configured_people_are_searchable() {
    let data = data();
    let found = data.people.search("rev", 5);
    assert_eq!(found.first().map(|c| c.name.as_ref()), Some("Rev"));
}

#[test]
fn org_live_initial_reflects_disabled_and_signed_out() {
    let mut disabled = data();
    disabled.profile.enabled = false;
    assert_eq!(OrgLive::initial(&disabled).auth_state, AuthState::Disabled);
    let mut signed_out = data();
    signed_out.signed_out = true;
    let live = OrgLive::initial(&signed_out);
    assert_eq!(live.auth_state, AuthState::SignedOut);
    assert_eq!(live.schedule.paused, Some(PauseReason::SignedOut));
    assert!(live.rows_stale());
}

#[test]
fn entra_config_requires_valid_client_id() {
    let mut profile = profile();
    assert_eq!(entra_config(&profile), None);
    profile.entra = EntraSettings {
        client_id: "not-a-guid".into(),
        tenant: String::new(),
    };
    assert_eq!(entra_config(&profile), None);
    profile.entra = EntraSettings {
        client_id: "00000000-0000-0000-0000-0000000000c1".into(),
        tenant: " contoso.onmicrosoft.com ".into(),
    };
    let config = entra_config(&profile).unwrap();
    assert_eq!(config.client_id, uuid::Uuid::from_u128(0xc1));
    assert_eq!(config.tenant.as_deref(), Some("contoso.onmicrosoft.com"));
}
