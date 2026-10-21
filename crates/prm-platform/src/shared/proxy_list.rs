//! Parsing a WinHTTP proxy list into the one proxy URL to use for https.

/// Entries are separated by `;` or whitespace, each `[scheme=]host[:port]` or
/// `[scheme://]host[:port]`. Prefers the `https=` entry, then the first unprefixed one, then
/// `http=`. `DIRECT` or nothing usable is `None`. A missing scheme means `http://`.
pub(crate) fn parse_proxy_list(list: &str) -> Option<url::Url> {
    let entries: Vec<(Option<&str>, &str)> = list
        .split(|c: char| c == ';' || c.is_whitespace())
        .filter(|e| !e.is_empty())
        .map(|entry| match entry.split_once('=') {
            Some((scheme, address)) => (Some(scheme), address),
            None => (None, entry),
        })
        .collect();
    let pick = |wanted: Option<&str>| {
        entries
            .iter()
            .find(|(scheme, _)| scheme.map(str::to_ascii_lowercase).as_deref() == wanted)
            .map(|(_, address)| *address)
    };
    let address = pick(Some("https"))
        .or_else(|| pick(None))
        .or_else(|| pick(Some("http")))?;
    if address.eq_ignore_ascii_case("direct") {
        return None;
    }
    let with_scheme = if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}")
    };
    url::Url::parse(&with_scheme)
        .ok()
        .filter(|url| url.host().is_some())
}

#[cfg(test)]
mod tests;
