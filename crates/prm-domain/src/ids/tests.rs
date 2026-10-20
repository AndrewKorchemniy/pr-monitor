use super::*;

fn org(s: &str) -> OrgId {
    OrgId::parse(s).unwrap()
}

#[test]
fn org_input_bare_name() {
    assert_eq!(parse_org_input(" Contoso ").unwrap(), org("contoso"));
    assert!(matches!(
        parse_org_input("-bad"),
        Err(OrgIdError::Invalid(_))
    ));
    assert_eq!(parse_org_input(""), Err(OrgIdError::Empty));
}

#[test]
fn org_input_dev_azure_url_with_path() {
    assert_eq!(
        parse_org_input("dev.azure.com/Fabrikam").unwrap(),
        org("fabrikam")
    );
    assert_eq!(
        parse_org_input("https://dev.azure.com/contoso/Proj/_git/repo/pullrequest/4").unwrap(),
        org("contoso")
    );
    assert_eq!(
        parse_org_input("https://vssps.dev.azure.com/contoso").unwrap(),
        org("contoso")
    );
}

#[test]
fn org_input_visualstudio_url() {
    assert_eq!(
        parse_org_input("https://fabrikam.visualstudio.com/DefaultCollection").unwrap(),
        org("fabrikam")
    );
    assert!(matches!(
        parse_org_input("https://app.visualstudio.com"),
        Err(OrgIdError::UnsupportedHost(_))
    ));
}

#[test]
fn org_input_rejects_unknown_host() {
    assert!(matches!(
        parse_org_input("https://github.com/contoso"),
        Err(OrgIdError::UnsupportedHost(_))
    ));
}

#[test]
fn prkey_roundtrip_display_fromstr() {
    let k = PrKey::new(org("contoso"), RepoId::from_u128(7), PrNumber(4711));
    let s = k.to_string();
    assert_eq!(s, "contoso/00000000-0000-0000-0000-000000000007/4711");
    assert_eq!(s.parse::<PrKey>().unwrap(), k);
    let json = serde_json::to_string(&k).unwrap();
    assert_eq!(serde_json::from_str::<PrKey>(&json).unwrap(), k);
    assert!("contoso/nope/1".parse::<PrKey>().is_err());
}

#[test]
fn prkey_tag_len_le_64_and_stable() {
    let k = PrKey::new(org("contoso"), RepoId::from_u128(7), PrNumber(4711));
    let t = k.tag();
    assert!(t.len() <= 64);
    assert_eq!(t.len(), 19);
    assert_eq!(t, k.clone().tag());
    assert!(t.starts_with("pr-"));
}

#[test]
fn vsid_parses_unhyphenated_and_uppercase() {
    let a: Vsid = "2428198325304A9CAEB788D60D57ACFD".parse().unwrap();
    let b: Vsid = "24281983-2530-4a9c-aeb7-88d60d57acfd".parse().unwrap();
    assert_eq!(a, b);
    assert_eq!(a.to_string(), "24281983-2530-4a9c-aeb7-88d60d57acfd");
}
