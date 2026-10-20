use prm_domain::testing::key;

use super::*;

fn show_uri() -> AppUri {
    AppUri::Show {
        key: Some(key("contoso", 1, 22)),
    }
}

fn decode(text: &str) -> Result<HandoffRequest, CodecError> {
    decode_request(text.as_bytes())
}

#[test]
fn handoff_roundtrip_show() {
    assert_eq!(
        decode_request(&encode_request(&HandoffRequest::Show)),
        Ok(HandoffRequest::Show)
    );
}

#[test]
fn handoff_roundtrip_activate() {
    let request = HandoffRequest::Activate(show_uri());
    assert_eq!(decode_request(&encode_request(&request)), Ok(request));
}

#[test]
fn handoff_roundtrip_quit() {
    assert_eq!(
        decode_request(&encode_request(&HandoffRequest::Quit)),
        Ok(HandoffRequest::Quit)
    );
}

#[test]
fn handoff_rejects_missing_newline() {
    assert_eq!(
        decode(r#"{"v":1,"cmd":"show"}"#),
        Err(CodecError::NoNewline)
    );
}

#[test]
fn handoff_rejects_oversize_frame() {
    let frame = format!("{}\n", "x".repeat(MAX_FRAME + 1));
    assert_eq!(decode(&frame), Err(CodecError::TooLong));
}

#[test]
fn handoff_rejects_unknown_cmd() {
    assert!(matches!(
        decode("{\"v\":1,\"cmd\":\"rm\"}\n"),
        Err(CodecError::Json(_))
    ));
}

#[test]
fn handoff_rejects_unknown_field() {
    let frame = "{\"v\":1,\"cmd\":\"show\",\"extra\":1}\n";
    assert!(matches!(decode(frame), Err(CodecError::Json(_))));
}

#[test]
fn handoff_rejects_wrong_version() {
    assert_eq!(
        decode("{\"v\":2,\"cmd\":\"show\"}\n"),
        Err(CodecError::Version(2))
    );
}

#[test]
fn handoff_rejects_uri_on_show() {
    let frame = "{\"v\":1,\"cmd\":\"show\",\"uri\":\"prmonitor://show\"}\n";
    assert_eq!(decode(frame), Err(CodecError::UnexpectedUri));
}

#[test]
fn handoff_rejects_missing_uri_on_activate() {
    assert_eq!(
        decode("{\"v\":1,\"cmd\":\"activate\"}\n"),
        Err(CodecError::MissingUri)
    );
}

#[test]
fn handoff_rejects_foreign_scheme_and_host() {
    for uri in ["https://evil.example/show", "prmonitor://delete-everything"] {
        let frame = format!("{{\"v\":1,\"cmd\":\"activate\",\"uri\":\"{uri}\"}}\n");
        assert!(matches!(decode(&frame), Err(CodecError::Uri(_))), "{uri}");
    }
}

#[test]
fn handoff_rejects_non_utf8_and_nul() {
    assert_eq!(decode_request(b"\xff\xfe\n"), Err(CodecError::NotUtf8));
    assert_eq!(
        decode("{\"v\":1,\0\"cmd\":\"show\"}\n"),
        Err(CodecError::Nul)
    );
}

#[test]
fn handoff_reply_roundtrip() {
    for reply in [HandoffReply::Ok, HandoffReply::Rejected("invalid uri".into())] {
        assert_eq!(decode_reply(&encode_reply(&reply)), Ok(reply.clone()));
    }
}

#[test]
fn handoff_wire_snapshot() {
    let lines = [
        encode_request(&HandoffRequest::Show),
        encode_request(&HandoffRequest::Activate(show_uri())),
        encode_request(&HandoffRequest::Quit),
        encode_reply(&HandoffReply::Ok),
        encode_reply(&HandoffReply::Rejected("invalid uri".into())),
    ]
    .map(|bytes| String::from_utf8(bytes).unwrap());
    insta::assert_snapshot!(lines.concat());
}
