//! Removes secrets from log text: bearer/basic tokens, token-like keys, JWTs and PATs. No regex;
//! returns the input unchanged (borrowed) when there is nothing to hide.

use std::borrow::Cow;

const KEYS: [&str; 8] = [
    "access_token",
    "refresh_token",
    "id_token",
    "code_verifier",
    "client_secret",
    "password",
    "accessToken",
    "code",
];
const SCHEMES: [&str; 2] = ["bearer ", "basic "];
/// Shortest token run after a scheme that counts as a secret.
const MIN_TOKEN: usize = 8;
const MIN_JWT: usize = 20;
const PAT_SHORT: usize = 52;
const PAT_LONG: usize = 84;
/// Where the "AZDO" signature sits in a long PAT.
const PAT_SIGNATURE: std::ops::Range<usize> = 76..80;

pub(crate) fn redact(input: &str) -> Cow<'_, str> {
    let mut text = input.to_owned();
    let mut changed = false;
    changed |= schemes(&mut text);
    changed |= keys(&mut text);
    changed |= jwts(&mut text);
    changed |= pats(&mut text);
    if changed {
        Cow::Owned(text)
    } else {
        Cow::Borrowed(input)
    }
}

const fn is_token_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

const fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Replaces `text[start..end]` and returns where scanning continues.
fn splice(text: &mut String, start: usize, end: usize, with: &str) -> usize {
    text.replace_range(start..end, with);
    start + with.len()
}

/// The end of a run of bytes matching `keep`, from `start`.
fn run_end(bytes: &[u8], start: usize, keep: impl Fn(u8) -> bool) -> usize {
    bytes[start..]
        .iter()
        .position(|b| !keep(*b))
        .map_or(bytes.len(), |n| start + n)
}

/// `Bearer abc…` / `Basic abc…` (case-insensitive) with at least 8 token characters.
fn schemes(text: &mut String) -> bool {
    let mut changed = false;
    let mut at = 0;
    while at < text.len() {
        let lower = text[at..].to_ascii_lowercase();
        let Some((offset, scheme)) = SCHEMES
            .iter()
            .filter_map(|s| lower.find(s).map(|i| (i, s)))
            .min()
        else {
            break;
        };
        let start = at + offset + scheme.len();
        let end = run_end(text.as_bytes(), start, is_token_char);
        at = if end - start >= MIN_TOKEN {
            changed = true;
            splice(text, start, end, "***")
        } else {
            start
        };
    }
    changed
}

/// `key=value`, `"key":"value"` and `"key": "value"` for secret-bearing keys.
fn keys(text: &mut String) -> bool {
    let mut changed = false;
    for key in KEYS {
        let mut at = 0;
        while let Some(found) = text[at..].find(key).map(|i| at + i) {
            at = found + key.len();
            let bytes = text.as_bytes();
            let bounded = found == 0 || !is_word(bytes[found - 1]);
            let Some(value_start) = value_start(bytes, at).filter(|_| bounded) else {
                continue;
            };
            let end = run_end(bytes, value_start, |b| {
                !matches!(b, b'&' | b'"' | b'\'' | b',' | b'}') && !b.is_ascii_whitespace()
            });
            if end > value_start {
                changed = true;
                at = splice(text, value_start, end, "***");
            }
        }
    }
    changed
}

/// Where a value starts after a key ending at `at`: `=`, `":"` or `": "`.
fn value_start(bytes: &[u8], at: usize) -> Option<usize> {
    let rest = &bytes[at..];
    [&b"="[..], b"\":\"", b"\": \""]
        .iter()
        .find(|sep| rest.starts_with(sep))
        .map(|sep| at + sep.len())
}

/// `eyJ<header>.<payload>.<signature>` at a word boundary → `eyJ***`.
fn jwts(text: &mut String) -> bool {
    let mut changed = false;
    let mut at = 0;
    while let Some(found) = text[at..].find("eyJ").map(|i| at + i) {
        let bytes = text.as_bytes();
        let bounded = found == 0 || !is_word(bytes[found - 1]);
        let end = run_end(bytes, found, |b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
        });
        let dots = text[found..end].matches('.').count();
        at = if bounded && dots == 2 && end - found >= MIN_JWT {
            changed = true;
            splice(text, found, end, "eyJ***")
        } else {
            found + 3
        };
    }
    changed
}

/// Alphanumeric runs of exactly 52, or 84 with "AZDO" at 76..80 → `***PAT***`.
fn pats(text: &mut String) -> bool {
    let mut changed = false;
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
            changed = true;
            splice(text, start, end, "***PAT***")
        } else {
            end
        };
    }
    changed
}

#[cfg(test)]
mod tests;
