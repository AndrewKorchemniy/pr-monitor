use super::*;
use crate::testing::key;

#[test]
fn app_uri_roundtrip() {
    let cases = [
        AppUri::Show { key: None },
        AppUri::Show {
            key: Some(key("contoso", 7, 4711)),
        },
        AppUri::Refresh,
    ];
    for uri in cases {
        assert_eq!(AppUri::parse(&uri.to_string()), Ok(uri.clone()), "{uri}");
    }
}

#[test]
fn app_uri_scheme_is_case_insensitive() {
    assert_eq!(AppUri::parse("PRMonitor://refresh"), Ok(AppUri::Refresh));
}

#[test]
fn app_uri_rejects_unknown_host_and_keys() {
    assert!(matches!(
        AppUri::parse("prmonitor://delete"),
        Err(AppUriError::Host(_))
    ));
    assert!(matches!(
        AppUri::parse("prmonitor://show?cmd=x"),
        Err(AppUriError::Key(_))
    ));
    assert_eq!(
        AppUri::parse("prmonitor://show?org=contoso"),
        Err(AppUriError::PartialKey)
    );
    assert_eq!(AppUri::parse("https://show"), Err(AppUriError::Scheme));
    assert!(matches!(
        AppUri::parse("prmonitor://show?org=contoso&repo=nope&pr=1"),
        Err(AppUriError::Invalid(_))
    ));
}

#[test]
fn app_uri_rejects_overlong() {
    let long = format!("prmonitor://show?{}", "x".repeat(MAX_URI_LEN));
    assert_eq!(AppUri::parse(&long), Err(AppUriError::TooLong));
}
