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
    for reply in [
        HandoffReply::Ok,
        HandoffReply::Rejected("invalid uri".into()),
    ] {
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

/// Bytes a mutation may insert: JSON syntax, quotes, escapes, NUL, newline and invalid UTF-8.
const MUTATION_BYTES: &[u8] = b"{}[]\":,\\/ \n\0x1v0cmdurishow\x7f\xc3\xa9\xff\xfe";

/// One random edit: insert, delete, replace, duplicate a run, or truncate.
fn mutate(rng: &mut fastrand::Rng, frame: &mut Vec<u8>) {
    let at = rng.usize(..=frame.len());
    let pick = |rng: &mut fastrand::Rng| MUTATION_BYTES[rng.usize(..MUTATION_BYTES.len())];
    match rng.u8(..5) {
        0 => frame.insert(at, pick(rng)),
        1 if at < frame.len() => drop(frame.remove(at)),
        2 if at < frame.len() => frame[at] = pick(rng),
        3 => {
            let end = rng.usize(at..=frame.len());
            let run = frame[at..end].to_vec();
            frame.splice(at..at, run);
        }
        _ => frame.truncate(at),
    }
}

#[test]
fn handoff_never_panics_on_mutations() {
    let seeds = [
        encode_request(&HandoffRequest::Show),
        encode_request(&HandoffRequest::Quit),
        encode_request(&HandoffRequest::Activate(show_uri())),
        encode_reply(&HandoffReply::Ok),
        encode_reply(&HandoffReply::Rejected("invalid uri".into())),
    ];
    let mut rng = fastrand::Rng::with_seed(0x5eed);
    for _ in 0..2000 {
        let mut frame = seeds[rng.usize(..seeds.len())].clone();
        for _ in 0..rng.usize(1..=4) {
            mutate(&mut rng, &mut frame);
        }
        if let Ok(request) = decode_request(&frame) {
            let again = decode_request(&encode_request(&request));
            assert_eq!(again, Ok(request), "{frame:?}");
        }
        if let Ok(reply) = decode_reply(&frame) {
            assert_eq!(decode_reply(&encode_reply(&reply)), Ok(reply), "{frame:?}");
        }
    }
}
