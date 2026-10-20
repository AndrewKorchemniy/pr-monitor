use super::*;

#[test]
fn classify_session_table() {
    let cases = [
        (false, false, Some("Console"), SessionKind::Local),
        (false, false, None, SessionKind::Local),
        (true, false, Some("RDP-Tcp#4"), SessionKind::Rdp),
        (false, true, Some("RDP-Tcp#4"), SessionKind::Rdp),
        (true, false, None, SessionKind::OtherRemote),
        (false, false, Some("TC-1"), SessionKind::ThinClient),
        (true, false, Some("tc-2"), SessionKind::ThinClient),
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
fn remote_agent_host_counts_as_remote() {
    let info = SessionInfo {
        kind: SessionKind::Local,
        id: Some(1),
        remote_agent_host: true,
    };
    assert!(info.is_remote());
    let local = SessionInfo {
        remote_agent_host: false,
        ..info
    };
    assert!(!local.is_remote());
}
