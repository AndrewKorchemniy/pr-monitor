//! UTC timestamps. Serialized as RFC 3339 strings.

use std::{fmt, time::Duration};

use ::time::{Date, OffsetDateTime, Time, UtcOffset, format_description::well_known::Rfc3339};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A point in time. Invariant: the inner value is in UTC.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    pub const UNIX_EPOCH: Self = Self(OffsetDateTime::UNIX_EPOCH);
    /// Saturation bound for additions that overflow.
    pub const FAR_FUTURE: Self = Self(OffsetDateTime::new_utc(Date::MAX, Time::MIDNIGHT));
    /// Saturation bound for subtractions that overflow.
    pub const FAR_PAST: Self = Self(OffsetDateTime::new_utc(Date::MIN, Time::MIDNIGHT));

    pub fn from_unix(secs: i64) -> Option<Self> {
        OffsetDateTime::from_unix_timestamp(secs).ok().map(Self)
    }

    pub fn unix(self) -> i64 {
        self.0.unix_timestamp()
    }

    /// Converts any offset to UTC.
    pub fn from_offset(dt: OffsetDateTime) -> Self {
        Self(dt.to_offset(UtcOffset::UTC))
    }

    pub const fn inner(self) -> OffsetDateTime {
        self.0
    }

    /// Azure DevOps dates: RFC 3339 with 1..=7 fraction digits. Offset-less values are read as UTC.
    /// Year <= 1 (the unset `0001-01-01T00:00:00`) yields `None`.
    pub fn parse_ado(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let parsed = OffsetDateTime::parse(s, &Rfc3339)
            .or_else(|_| OffsetDateTime::parse(&format!("{s}Z"), &Rfc3339))
            .ok()?;
        (parsed.year() > 1).then(|| Self::from_offset(parsed))
    }

    pub fn parse_rfc3339(s: &str) -> Option<Self> {
        OffsetDateTime::parse(s, &Rfc3339)
            .ok()
            .map(Self::from_offset)
    }

    pub fn to_rfc3339(self) -> String {
        self.0
            .format(&Rfc3339)
            .unwrap_or_else(|_| self.0.unix_timestamp().to_string())
    }

    pub fn saturating_add(self, d: Duration) -> Self {
        ::time::Duration::try_from(d)
            .ok()
            .and_then(|d| self.0.checked_add(d))
            .map_or(Self::FAR_FUTURE, Self)
    }

    pub fn saturating_sub(self, d: Duration) -> Self {
        ::time::Duration::try_from(d)
            .ok()
            .and_then(|d| self.0.checked_sub(d))
            .map_or(Self::FAR_PAST, Self)
    }

    /// Elapsed time since `earlier`; zero if `earlier` is later than `self`.
    pub fn since(self, earlier: Self) -> Duration {
        Duration::try_from(self.0 - earlier.0).unwrap_or(Duration::ZERO)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let text = self.0.format(&Rfc3339).map_err(serde::ser::Error::custom)?;
        s.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse_rfc3339(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid RFC 3339 timestamp: {s}")))
    }
}

#[cfg(test)]
mod tests;
