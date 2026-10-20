//! Small text helpers shared by rows, toasts and the tray tooltip.

/// The ellipsis appended when text is cut.
pub const ELLIPSIS: char = '…';
/// Comment snippets are cut to this many characters.
pub const MAX_SNIPPET_CHARS: usize = 160;

/// Cuts to at most `max` characters (not bytes), appending `…` when cut.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}{ELLIPSIS}")
}

/// Cuts to at most `max_units` UTF-16 code units (Windows tooltip limits), never splitting a
/// surrogate pair, appending `…` when cut.
pub fn truncate_utf16(s: &str, max_units: usize) -> String {
    if s.encode_utf16().count() <= max_units {
        return s.to_owned();
    }
    let budget = max_units.saturating_sub(ELLIPSIS.len_utf16());
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        used += c.len_utf16();
        if used > budget {
            break;
        }
        out.push(c);
    }
    out.push(ELLIPSIS);
    out
}

/// Up to two uppercase initials: "Jane Doe" → "JD", `"[Team]\Payments"` → "P".
pub fn initials(name: &str) -> String {
    let words = name
        .rsplit('\\')
        .next()
        .unwrap_or(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty());
    words
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// "refs/heads/feature/x" → "feature/x".
pub fn strip_ref_prefix(name: &str) -> &str {
    name.strip_prefix("refs/heads/").unwrap_or(name)
}

/// The first non-empty line of a comment, without leading markdown markers, cut to
/// [`MAX_SNIPPET_CHARS`]. `None` when nothing visible remains.
pub fn snippet(content: &str) -> Option<String> {
    let line = content
        .lines()
        .map(|l| {
            l.trim_start_matches(|c: char| matches!(c, '#' | '>' | '*' | '-') || c.is_whitespace())
        })
        .map(str::trim_end)
        .find(|l| !l.is_empty())?;
    Some(truncate_chars(line, MAX_SNIPPET_CHARS))
}

#[cfg(test)]
mod tests;
