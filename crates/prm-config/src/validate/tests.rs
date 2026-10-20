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
    assert_eq!(config.orgs[1].label, "fabrikam");
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

#[test]
fn iso_date_table() {
    for (text, ok) in [
        ("2026-12-01", true),
        ("2026-13-01", false),
        ("26-12-01", false),
        ("tomorrow", false),
    ] {
        assert_eq!(is_iso_date(text), ok, "{text}");
    }
}
