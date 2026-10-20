use std::borrow::Cow;

use super::*;

#[test]
fn clean_text_is_borrowed_unchanged() {
    let text = "refresh done in 120 ms for contoso (repo 1f0e6c1a-2b3c-4d5e-8f90-0123456789ab)";
    assert!(matches!(redact(text), Cow::Borrowed(_)));
}

#[test]
fn bearer_and_basic_tokens_are_hidden() {
    assert_eq!(
        redact("Authorization: Bearer abc.def-123_xyz"),
        "Authorization: Bearer ***"
    );
    assert_eq!(
        redact("authorization: basic OnBhdHBhdHBhdA=="),
        "authorization: basic ***"
    );
    assert_eq!(
        redact("Bearer short"),
        "Bearer short",
        "too short to be a token"
    );
}

#[test]
fn secret_keys_in_queries_and_json_are_hidden() {
    assert_eq!(redact("code=abc&state=xyz"), "code=***&state=xyz");
    assert_eq!(
        redact(r#"{"access_token":"t0k3n","expires_in":3599}"#),
        r#"{"access_token":"***","expires_in":3599}"#
    );
    assert_eq!(
        redact(r#"{"refresh_token": "r3fr3sh"}"#),
        r#"{"refresh_token": "***"}"#
    );
    assert_eq!(redact("error_code=42"), "error_code=42", "only whole keys");
}

#[test]
fn jwts_and_pats_are_hidden() {
    let jwt = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJhIn0.c2ln";
    assert_eq!(redact(&format!("token {jwt} end")), "token eyJ*** end");
    let pat = "a".repeat(52);
    assert_eq!(redact(&format!("pat {pat}.")), "pat ***PAT***.");
    let long = format!("{}AZDO{}", "b".repeat(76), "c".repeat(4));
    assert_eq!(redact(&long), "***PAT***");
}

#[test]
fn hashes_and_guids_survive() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let guid = "1f0e6c1a-2b3c-4d5e-8f90-0123456789ab";
    let text = format!("commit {sha} repo {guid} fp 0123456789abcdef");
    assert_eq!(redact(&text), text);
}
