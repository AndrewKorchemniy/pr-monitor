use super::*;

#[test]
fn classify_session_table() {
    let cases = [
        (false, false, Some("Console"), SessionKind::Local),
        (false, false, None, SessionKind::Local),
        (true, false, Some("RDP-Tcp#4"), SessionKind::Rdp),
        (false, true, Some("RDP-Tcp#4"), SessionKind::Rdp),
        (true, false, None, SessionKind::OtherRemote),
        (true, false, Some("Remote#2"), SessionKind::OtherRemote),
    ];
    for (sm_remote, glass, name, expected) in cases {
        assert_eq!(
            classify_session(sm_remote, glass, name),
            expected,
            "{sm_remote} {glass} {name:?}"
        );
    }
}

#[test]
fn only_remote_kinds_count_as_remote() {
    let local = SessionInfo {
        kind: SessionKind::Local,
        id: Some(1),
    };
    assert!(!local.is_remote());
    for kind in [SessionKind::Rdp, SessionKind::OtherRemote] {
        assert!(SessionInfo { kind, ..local }.is_remote(), "{kind:?}");
    }
}
