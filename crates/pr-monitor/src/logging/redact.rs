//! Removes secrets from log text: bearer/basic tokens, token-like keys, JWTs and PATs. No regex;
//! returns the input unchanged (borrowed, no copy) when there is nothing to hide.

use std::borrow::Cow;

/// Secret-bearing keys, matched ignoring ASCII case (`accessToken`, `PASSWORD=`).
const KEYS: [&str; 8] = [
    "access_token",
    "refresh_token",
    "id_token",
    "code_verifier",
    "client_secret",
    "password",
    "accesstoken",
    "code",
];
/// What may sit between a key and its value: `=` (an opening quote is skipped), JSON `":"`,
/// and Rust `Debug` output `key: "…"`. The `: ` form needs the quote, so `code: 404` stays.
const SEPARATORS: [&[u8]; 6] = [b"=\"", b"=", b"\":\"", b"\": \"", b"\" : \"", b": \""];
const SCHEMES: [&str; 2] = ["bearer ", "basic "];
/// Shortest token run after a scheme that counts as a secret.
const MIN_TOKEN: usize = 8;
const MIN_JWT: usize = 20;
const PAT_SHORT: usize = 52;
const PAT_LONG: usize = 84;
/// Where the "AZDO" signature sits in a long PAT.
const PAT_SIGNATURE: std::ops::Range<usize> = 76..80;

pub(crate) fn redact(input: &str) -> Cow<'_, str> {
    let mut text = Cow::Borrowed(input);
    schemes(&mut text);
    keys(&mut text);
    jwts(&mut text);
    pats(&mut text);
    text
}

const fn is_token_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

const fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

const fn is_base64url(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-')
}

const fn is_value_char(c: u8) -> bool {
    !matches!(c, b'&' | b'"' | b'\'' | b',' | b'}') && !c.is_ascii_whitespace()
}

/// Replaces `text[start..end]` (copying the input on the first change) and returns where
/// scanning continues.
fn splice(text: &mut Cow<'_, str>, start: usize, end: usize, with: &str) -> usize {
    text.to_mut().replace_range(start..end, with);
    start + with.len()
}

/// The end of a run of bytes matching `keep`, from `start`.
fn run_end(bytes: &[u8], start: usize, keep: impl Fn(u8) -> bool) -> usize {
    bytes[start..]
        .iter()
        .position(|b| !keep(*b))
        .map_or(bytes.len(), |n| start + n)
}

/// The first `needle` (ASCII) at or after `from`, ignoring ASCII case, without allocating.
fn find_ignore_case(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
        .map(|i| from + i)
}

/// `Bearer abc…` / `Basic abc…` (case-insensitive) with at least 8 token characters.
fn schemes(text: &mut Cow<'_, str>) {
    let mut at = 0;
    while let Some((found, scheme)) = SCHEMES
        .iter()
        .filter_map(|s| find_ignore_case(text.as_bytes(), at, s.as_bytes()).map(|i| (i, s)))
        .min()
    {
        let start = found + scheme.len();
        let end = run_end(text.as_bytes(), start, is_token_char);
        at = if end - start >= MIN_TOKEN {
            splice(text, start, end, "***")
        } else {
            start
        };
    }
}

/// `key=value`, `key="value"`, `"key":"value"`, `"key": "value"` and `key: "value"` for
/// secret-bearing keys.
fn keys(text: &mut Cow<'_, str>) {
    for key in KEYS {
        let mut at = 0;
        while let Some(found) = find_ignore_case(text.as_bytes(), at, key.as_bytes()) {
            at = found + key.len();
            let bytes = text.as_bytes();
            let bounded = found == 0 || !is_word(bytes[found - 1]);
            let Some(value_start) = value_start(bytes, at).filter(|_| bounded) else {
                continue;
            };
            let end = run_end(bytes, value_start, is_value_char);
            if end > value_start {
                at = splice(text, value_start, end, "***");
            }
        }
    }
}

/// Where a value starts after a key ending at `at`.
fn value_start(bytes: &[u8], at: usize) -> Option<usize> {
    let rest = bytes.get(at..)?;
    SEPARATORS
        .iter()
        .find(|sep| rest.starts_with(sep))
        .map(|sep| at + sep.len())
}

/// The end of `<header>.<payload>.<signature>` from `start`: three non-empty base64url
/// segments joined by two dots, so trailing punctuation or a following dotted word is kept.
fn jwt_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut end = start;
    for segment in 0..3 {
        if segment > 0 {
            end = (bytes.get(end) == Some(&b'.')).then_some(end + 1)?;
        }
        let segment_end = run_end(bytes, end, is_base64url);
        if segment_end == end {
            return None;
        }
        end = segment_end;
    }
    Some(end)
}

/// `eyJ<header>.<payload>.<signature>` at a word boundary → `eyJ***`.
fn jwts(text: &mut Cow<'_, str>) {
    let mut at = 0;
    while let Some(found) = text[at..].find("eyJ").map(|i| at + i) {
        let bytes = text.as_bytes();
        let bounded = found == 0 || !is_word(bytes[found - 1]);
        let end = jwt_end(bytes, found).filter(|end| bounded && end - found >= MIN_JWT);
        at = match end {
            Some(end) => splice(text, found, end, "eyJ***"),
            None => found + 3,
        };
    }
}

/// Alphanumeric runs of exactly 52, or 84 with "AZDO" at 76..80 → `***PAT***`.
fn pats(text: &mut Cow<'_, str>) {
    let mut at = 0;
    while at < text.len() {
        let bytes = text.as_bytes();
        let Some(start) = bytes[at..]
            .iter()
            .position(u8::is_ascii_alphanumeric)
            .map(|i| at + i)
        else {
            break;
        };
        let end = run_end(bytes, start, |b| b.is_ascii_alphanumeric());
        let run = &text[start..end];
        let is_pat =
            run.len() == PAT_SHORT || (run.len() == PAT_LONG && &run[PAT_SIGNATURE] == "AZDO");
        at = if is_pat {
            splice(text, start, end, "***PAT***")
        } else {
            end
        };
    }
}

#[cfg(test)]
mod tests;
