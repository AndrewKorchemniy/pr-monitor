use super::*;

#[test]
fn openable_hosts() {
    let ok = |s: &str| is_openable_url(&Url::parse(s).unwrap());
    assert!(ok("https://dev.azure.com/contoso/_git/r/pullrequest/1"));
    assert!(ok("https://contoso.visualstudio.com/x"));
    assert!(ok("https://GitHub.com/AndrewKorchemniy/pr-monitor"));
    assert!(!ok("http://dev.azure.com/contoso"));
    assert!(!ok("https://visualstudio.com/x"));
    assert!(!ok("https://evil.com/dev.azure.com"));
    assert!(!ok("https://dev.azure.com.evil.com/"));
    assert!(!ok("file:///c:/windows"));
}

#[test]
fn doc_links_are_openable() {
    for d in [
        DocLink::InstallAzureCli,
        DocLink::CreatePat,
        DocLink::EntraAppRegistration,
        DocLink::Troubleshooting,
        DocLink::ReportProblem,
    ] {
        assert!(is_openable_url(&Url::parse(d.url()).unwrap()), "{d:?}");
    }
}

#[test]
fn user_agent_format() {
    assert_eq!(
        user_agent("1.2.3"),
        "PRMonitor/1.2.3 (+https://github.com/AndrewKorchemniy/pr-monitor)"
    );
}
