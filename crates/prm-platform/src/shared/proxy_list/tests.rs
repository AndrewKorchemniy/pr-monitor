use super::*;

fn parsed(list: &str) -> Option<String> {
    parse_proxy_list(list).map(|u| u.to_string())
}

#[test]
fn proxy_list_single() {
    assert_eq!(
        parsed("proxy.corp:8080"),
        Some("http://proxy.corp:8080/".into())
    );
}

#[test]
fn proxy_list_first_of_many() {
    assert_eq!(
        parsed("first.corp:8080; second.corp:3128"),
        Some("http://first.corp:8080/".into())
    );
}

#[test]
fn proxy_list_prefers_https_entry() {
    assert_eq!(
        parsed("http=web.corp:80;https=secure.corp:443"),
        Some("http://secure.corp:443/".into())
    );
    assert_eq!(parsed("http=web.corp:80"), Some("http://web.corp/".into()));
}

#[test]
fn proxy_list_direct_is_none() {
    assert_eq!(parsed("DIRECT"), None);
    assert_eq!(parsed(""), None);
    assert_eq!(parsed(" ;; "), None);
}

#[test]
fn proxy_list_adds_http_scheme() {
    assert_eq!(
        parsed("socks.corp:1080"),
        Some("http://socks.corp:1080/".into())
    );
    assert_eq!(
        parsed("https://tls-proxy.corp:8443"),
        Some("https://tls-proxy.corp:8443/".into())
    );
}

#[test]
fn proxy_list_ipv6() {
    assert_eq!(parsed("[::1]:8080"), Some("http://[::1]:8080/".into()));
}

#[test]
fn proxy_list_garbage_none() {
    assert_eq!(parsed("http://"), None);
    assert_eq!(parsed(":8080"), None);
}
