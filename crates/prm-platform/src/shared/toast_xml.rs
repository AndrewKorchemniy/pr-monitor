//! Toast XML for Windows: a `Notification` becomes a deterministic, escaped `ToastGeneric`
//! payload with protocol activation only.

use prm_domain::{
    hash::fnv64,
    notify::{
        Notification,
        text::{ACTION_OPEN_PR, ACTION_SHOW},
    },
    view::text::truncate_chars,
};

pub const MAX_TITLE: usize = 128;
pub const MAX_BODY: usize = 512;
pub const MAX_DETAIL: usize = 256;
pub const MAX_ATTRIBUTION: usize = 128;
/// Windows rejects longer tags and groups.
pub const MAX_TAG_UTF16: usize = 64;

/// A button; always protocol-activated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastAction {
    pub content: String,
    pub arguments: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastSpec {
    pub launch: String,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
    pub attribution: Option<String>,
    pub actions: Vec<ToastAction>,
    pub silent: bool,
}

/// Clipped fields; "Open PR" and "Show" buttons only when the PR link is https.
pub fn spec_from(n: &Notification) -> ToastSpec {
    let open_url = n
        .open_url
        .as_deref()
        .and_then(|u| url::Url::parse(u).ok())
        .filter(|u| u.scheme() == "https");
    let actions = open_url.map_or_else(Vec::new, |url| {
        vec![
            ToastAction {
                content: ACTION_OPEN_PR.to_owned(),
                arguments: url.to_string(),
            },
            ToastAction {
                content: ACTION_SHOW.to_owned(),
                arguments: n.launch.clone(),
            },
        ]
    });
    ToastSpec {
        launch: n.launch.clone(),
        title: truncate_chars(&n.title, MAX_TITLE),
        body: truncate_chars(&n.body, MAX_BODY),
        detail: n.detail.as_deref().map(|d| truncate_chars(d, MAX_DETAIL)),
        attribution: n
            .attribution
            .as_deref()
            .map(|a| truncate_chars(a, MAX_ATTRIBUTION)),
        actions,
        silent: n.silent,
    }
}

/// Compact XML with no whitespace between elements.
pub fn toast_xml(spec: &ToastSpec) -> String {
    let mut out = String::with_capacity(512);
    out.push_str("<toast launch=\"");
    escape_xml(&spec.launch, &mut out);
    out.push_str("\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\">");
    text_element(&mut out, " hint-maxLines=\"1\"", &spec.title);
    text_element(&mut out, "", &spec.body);
    if let Some(detail) = &spec.detail {
        text_element(&mut out, "", detail);
    }
    if let Some(attribution) = &spec.attribution {
        text_element(&mut out, " placement=\"attribution\"", attribution);
    }
    out.push_str("</binding></visual>");
    if !spec.actions.is_empty() {
        out.push_str("<actions>");
        for action in &spec.actions {
            out.push_str("<action content=\"");
            escape_xml(&action.content, &mut out);
            out.push_str("\" activationType=\"protocol\" arguments=\"");
            escape_xml(&action.arguments, &mut out);
            out.push_str("\"/>");
        }
        out.push_str("</actions>");
    }
    if spec.silent {
        out.push_str("<audio silent=\"true\"/>");
    }
    out.push_str("</toast>");
    out
}

fn text_element(out: &mut String, attributes: &str, text: &str) {
    out.push_str("<text");
    out.push_str(attributes);
    out.push('>');
    escape_xml(text, out);
    out.push_str("</text>");
}

/// XML 1.0 `Char` production.
const fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

/// Escapes markup; characters XML cannot carry become U+FFFD.
pub fn escape_xml(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if is_xml_char(c) => out.push(c),
            _ => out.push(char::REPLACEMENT_CHARACTER),
        }
    }
}

/// Short, non-empty tags pass through; anything else becomes a stable hash.
pub fn sanitize_tag(tag: &str) -> String {
    let units = tag.encode_utf16().count();
    if units > 0 && units <= MAX_TAG_UTF16 {
        return tag.to_owned();
    }
    format!("h-{:016x}", fnv64(tag.as_bytes()))
}

#[cfg(test)]
mod tests;
