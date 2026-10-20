use prm_domain::notify::{GROUP_PRS, NotificationKind};

use super::*;

fn pr_toast() -> Notification {
    Notification {
        tag: "pr-0123456789abcdef".into(),
        group: GROUP_PRS.into(),
        title: "2 new comments · !22".into(),
        body: "A new feature".into(),
        detail: Some("Rev: looks good, one nit".into()),
        attribution: Some("api-gateway · Ann".into()),
        launch: "prmonitor://show?org=contoso&repo=00000000-0000-0000-0000-000000000001&pr=22"
            .into(),
        open_url: Some(
            "https://dev.azure.com/contoso/Payments/_git/api-gateway/pullrequest/22".into(),
        ),
        silent: false,
        kind: NotificationKind::Pr,
    }
}

#[test]
fn toast_xml_pr_with_open_url() {
    insta::assert_snapshot!(toast_xml(&spec_from(&pr_toast())));
}

#[test]
fn toast_xml_summary_without_actions() {
    let summary = Notification {
        tag: "summary".into(),
        group: "summary".into(),
        title: "5 pull requests changed".into(),
        body: "Fix token refresh, Paging terminator, and 3 more".into(),
        detail: None,
        attribution: None,
        launch: "prmonitor://show".into(),
        open_url: None,
        silent: true,
        kind: NotificationKind::Summary,
    };
    insta::assert_snapshot!(toast_xml(&spec_from(&summary)));
}

#[test]
fn toast_xml_escapes_markup_in_title_body_attribution() {
    let mut toast = pr_toast();
    toast.title = "<b>bold</b>".into();
    toast.body = "a & b".into();
    toast.attribution = Some("\"quoted\" 'single'".into());
    let xml = toast_xml(&spec_from(&toast));
    assert!(xml.contains("&lt;b&gt;bold&lt;/b&gt;"));
    assert!(xml.contains("a &amp; b"));
    assert!(xml.contains("&quot;quoted&quot; &apos;single&apos;"));
    assert!(!xml.contains("<b>"));
}

#[test]
fn toast_xml_escapes_ampersand_in_launch_and_arguments() {
    let xml = toast_xml(&spec_from(&pr_toast()));
    assert!(xml.contains("show?org=contoso&amp;repo="));
    assert!(!xml.contains("contoso&repo"));
}

#[test]
fn toast_xml_silent_emits_audio_silent() {
    let mut toast = pr_toast();
    assert!(!toast_xml(&spec_from(&toast)).contains("<audio"));
    toast.silent = true;
    assert!(toast_xml(&spec_from(&toast)).ends_with("<audio silent=\"true\"/></toast>"));
}

#[test]
fn toast_xml_drops_non_https_open_url() {
    for url in ["http://dev.azure.com/x", "javascript:alert(1)", "not a url"] {
        let mut toast = pr_toast();
        toast.open_url = Some(url.into());
        let spec = spec_from(&toast);
        assert_eq!(spec.actions, Vec::new(), "{url}");
        assert!(!toast_xml(&spec).contains("<actions>"), "{url}");
    }
}

#[test]
fn toast_xml_replaces_invalid_xml_chars() {
    let mut out = String::new();
    escape_xml("a\u{1}b\u{FFFE}c\td", &mut out);
    assert_eq!(out, "a\u{FFFD}b\u{FFFD}c\td");
}

#[test]
fn toast_xml_truncates_fields() {
    let mut toast = pr_toast();
    toast.title = "t".repeat(500);
    toast.body = "b".repeat(2_000);
    let spec = spec_from(&toast);
    assert_eq!(spec.title.chars().count(), MAX_TITLE);
    assert_eq!(spec.body.chars().count(), MAX_BODY);
}

#[test]
fn toast_tag_kept_when_short() {
    assert_eq!(sanitize_tag("pr-0123456789abcdef"), "pr-0123456789abcdef");
    assert_eq!(sanitize_tag(&"x".repeat(64)), "x".repeat(64));
}

#[test]
fn toast_tag_hashed_when_empty_or_over_64_utf16() {
    let long = "x".repeat(65);
    for tag in ["", long.as_str(), &"😀".repeat(33)] {
        let sanitized = sanitize_tag(tag);
        assert!(sanitized.starts_with("h-"), "{tag}");
        assert_eq!(sanitized.len(), 18);
    }
    assert_eq!(sanitize_tag(&long), sanitize_tag(&long), "stable");
}
