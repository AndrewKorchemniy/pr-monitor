use http::HeaderValue;

use super::*;

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(*name, HeaderValue::from_static(value));
    }
    map
}

#[test]
fn retry_after_seconds_and_decimals() {
    assert_eq!(
        parse_retry_after(&headers(&[("retry-after", "5")])),
        Some(Duration::from_secs(5))
    );
    assert_eq!(
        parse_retry_after(&headers(&[("retry-after", "1.5")])),
        Some(Duration::from_millis(1500))
    );
    assert_eq!(
        parse_retry_after(&headers(&[("retry-after", "999")])),
        Some(MAX_RETRY_AFTER)
    );
    assert_eq!(
        parse_retry_after(&headers(&[(
            "retry-after",
            "Wed, 21 Oct 2015 07:28:00 GMT"
        )])),
        None
    );
    assert_eq!(parse_retry_after(&HeaderMap::new()), None);
}

#[test]
fn ratelimit_headers_snapshot_headroom() {
    let gate = RateGate::default();
    gate.observe(&headers(&[
        ("x-ratelimit-limit", "200"),
        ("x-ratelimit-remaining", "50"),
        ("x-ratelimit-resource", "Core"),
    ]));
    let snap = gate.snapshot();
    assert_eq!(snap.headroom(), Some(0.25));
    assert_eq!(snap.resource.as_deref(), Some("Core"));
    assert_eq!(RateSnapshot::default().headroom(), None);
}

#[tokio::test(start_paused = true)]
async fn gate_waits_out_retry_after() {
    let gate = RateGate::default();
    gate.observe(&headers(&[("retry-after", "10")]));
    let start = Instant::now();
    gate.wait().await;
    assert!(start.elapsed() >= Duration::from_secs(10));
    let again = Instant::now();
    gate.wait().await;
    assert_eq!(again.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn gate_push_keeps_the_later_deadline() {
    let gate = RateGate::default();
    gate.push(Duration::from_secs(30));
    gate.push(Duration::from_secs(5));
    let start = Instant::now();
    gate.wait().await;
    assert!(start.elapsed() >= Duration::from_secs(30));
}
