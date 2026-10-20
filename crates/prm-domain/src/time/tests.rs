use super::*;

#[test]
fn parse_ado_seven_fraction_digits() {
    let t = Timestamp::parse_ado("2016-11-01T16:30:31.6655471Z").unwrap();
    assert_eq!(t.unix(), 1_478_017_831);
    assert_eq!(t.inner().nanosecond(), 665_547_100);
    let no_offset = Timestamp::parse_ado("2016-11-01T16:30:31.66").unwrap();
    assert_eq!(no_offset.unix(), 1_478_017_831);
}

#[test]
fn parse_ado_zero_date_is_none() {
    assert_eq!(Timestamp::parse_ado("0001-01-01T00:00:00"), None);
    assert_eq!(Timestamp::parse_ado("0001-01-01T00:00:00Z"), None);
    assert_eq!(Timestamp::parse_ado(""), None);
    assert_eq!(Timestamp::parse_ado("garbage"), None);
}

#[test]
fn timestamp_serde_rfc3339_roundtrip() {
    let t = Timestamp::parse_ado("2026-10-19T14:03:11.123Z").unwrap();
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(json, "\"2026-10-19T14:03:11.123Z\"");
    assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), t);
    let offset: Timestamp = serde_json::from_str("\"2026-10-19T16:03:11+02:00\"").unwrap();
    assert_eq!(offset.to_rfc3339(), "2026-10-19T14:03:11Z");
}

#[test]
fn arithmetic_saturates() {
    let t = Timestamp::from_unix(1_000).unwrap();
    assert_eq!(t.saturating_add(Duration::from_secs(10)).unix(), 1_010);
    assert_eq!(t.saturating_sub(Duration::from_secs(10)).unix(), 990);
    assert_eq!(
        t.since(t.saturating_add(Duration::from_secs(5))),
        Duration::ZERO
    );
    assert_eq!(
        t.saturating_add(Duration::from_secs(5)).since(t),
        Duration::from_secs(5)
    );
    let far = t.saturating_add(Duration::MAX);
    assert!(far > t);
}
