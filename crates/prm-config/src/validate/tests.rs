use super::*;
use crate::schema::ProxyConfig;

const INVALID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/config_invalid_values.toml"
));

fn invalid() -> (Config, Vec<ConfigIssue>) {
    normalize(toml::from_str(INVALID).unwrap())
}

fn issue_at<'a>(issues: &'a [ConfigIssue], path: &str) -> Option<&'a ConfigIssue> {
    issues.iter().find(|i| &*i.path == path)
}

#[test]
fn normalize_clamps() {
    let (config, issues) = invalid();
    for (path, value) in [
        (
            "refresh.interval_minutes",
            config.refresh.interval_minutes.to_string(),
        ),
        (
            "refresh.jitter_percent",
            config.refresh.jitter_percent.to_string(),
        ),
        (
            "notifications.max_per_refresh",
            config.notifications.max_per_refresh.to_string(),
        ),
        (
            "network.max_concurrency",
            config.network.max_concurrency.to_string(),
        ),
        ("ui.font_scale", config.ui.font_scale.to_string()),
    ] {
        let issue = issue_at(&issues, path).unwrap_or_else(|| panic!("no issue for {path}"));
        assert_eq!(issue.severity, Severity::Warning, "{path}");
        assert!(
            issue.message.ends_with(&format!("using {value}.")),
            "{path}: {}",
            issue.message
        );
    }
    assert_eq!(config.refresh.interval_minutes, 60);
    assert_eq!(config.network.proxy, ProxyConfig::System);
}

#[test]
fn normalize_bad_url_issue() {
    let (config, issues) = invalid();
    assert!(!config.orgs[0].enabled);
    assert_eq!(config.orgs[0].id, None);
    assert_eq!(
        issue_at(&issues, "orgs[0].url").map(|i| i.severity),
        Some(Severity::Error)
    );
}

#[test]
fn normalize_canonical_url() {
    let (config, _) = invalid();
    assert_eq!(config.orgs[1].url, "https://dev.azure.com/fabrikam");
}

/// A blank label used to be filled with the org id and saved, so it went stale when the URL
/// was later corrected; the fallback is computed by `display_label` instead.
#[test]
fn normalize_leaves_a_blank_label_blank() {
    let mut config = Config::default();
    config.orgs.push(OrgProfile {
        url: "contoso".into(),
        ..OrgProfile::default()
    });
    let (mut config, _) = normalize(config);
    assert_eq!(config.orgs[0].label, "");
    config.orgs[0].url = "fabrikam".into();
    let (config, _) = normalize(config);
    assert_eq!(&*config.orgs[0].display_label(), "fabrikam");
}

#[test]
fn normalize_duplicate_org_disabled() {
    let (config, issues) = invalid();
    assert!(config.orgs[1].enabled);
    assert!(!config.orgs[2].enabled);
    assert_eq!(
        issue_at(&issues, "orgs[2].url").map(|i| i.severity),
        Some(Severity::Error)
    );
}

#[test]
fn normalize_tenant_guid() {
    let (config, issues) = invalid();
    assert_eq!(config.orgs[1].tenant, "");
    assert!(issue_at(&issues, "orgs[1].tenant").is_some());
    assert!(issue_at(&issues, "orgs[1].pat_expires_on").is_some());
}

#[test]
fn normalize_first_party_client_id_info() {
    let (config, issues) = invalid();
    assert_eq!(
        config.orgs[1].entra.client_id,
        "04b07795-8ddb-461a-bbee-02f9e1bf7b46"
    );
    assert_eq!(
        issue_at(&issues, "orgs[1].entra.client_id").map(|i| i.severity),
        Some(Severity::Info)
    );
}

#[test]
fn normalize_dedupes_people() {
    let mut config = Config::default();
    let person = Person::new(prm_domain::Vsid::from_u128(1), "A");
    config.orgs.push(OrgProfile {
        url: "contoso".into(),
        people: crate::schema::PeopleConfig {
            reviewers: vec![person.clone(), person],
            ..Default::default()
        },
        ..Default::default()
    });
    let (config, _) = normalize(config);
    assert_eq!(config.orgs[0].people.reviewers.len(), 1);
}

/// `2025-02-30` used to pass normalize, then fail the reminder's own parse, so the PAT
/// expiry reminder silently never fired.
#[test]
fn normalize_clears_impossible_pat_expiry() {
    let mut config = Config::default();
    config.orgs.push(OrgProfile {
        url: "contoso".into(),
        pat_expires_on: "2025-02-30".into(),
        ..OrgProfile::default()
    });
    let (config, issues) = normalize(config);
    assert_eq!(config.orgs[0].pat_expires_on, "");
    assert!(issue_at(&issues, "orgs[0].pat_expires_on").is_some());
}

#[test]
fn normalize_non_finite_font_scale_uses_default() {
    let mut config = Config::default();
    config.ui.font_scale = f32::NAN;
    let (config, issues) = normalize(config);
    assert_eq!(config.ui.font_scale, limits::DEFAULT_FONT_SCALE);
    assert!(issue_at(&issues, "ui.font_scale").is_some());
}
