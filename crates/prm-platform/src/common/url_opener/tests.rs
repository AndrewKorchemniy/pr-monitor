use super::*;

fn open(url: &str) -> Result<(), PortError> {
    BrowserOpener.open(&url::Url::parse(url).unwrap())
}

#[test]
fn opener_denies_non_allowlisted_host() {
    let error = open("https://evil.example.com/pull/1").unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Denied);
}

#[test]
fn opener_denies_http_scheme() {
    let error = open("http://dev.azure.com/contoso").unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Denied);
}
