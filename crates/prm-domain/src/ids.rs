//! Identifier newtypes. Azure DevOps identities and repos are GUIDs; orgs are lowercase names.

use std::{fmt, str::FromStr, sync::Arc};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::hash::fnv64;

/// Declares a GUID newtype with Display/Debug/FromStr. A macro because three identical
/// types (Vsid, ProjectId, RepoId) need the same ~40 lines and distinct types are the point.
macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub uuid::Uuid);

        impl $name {
            pub const fn from_u128(v: u128) -> Self {
                Self(uuid::Uuid::from_u128(v))
            }

            pub const fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.hyphenated())
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            /// Accepts hyphenated, un-hyphenated, braced and urn forms in any case.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(s.trim()).map(Self)
            }
        }
    };
}

uuid_id!(
    /// An Azure DevOps identity id (`IdentityRef.id`, a.k.a. VSID or storage key).
    Vsid
);
uuid_id!(ProjectId);
uuid_id!(RepoId);

/// Declares an integer newtype with Display. A macro for the same reason as `uuid_id!`:
/// four id types share it.
macro_rules! int_id {
    ($(#[$meta:meta])* $name:ident($inner:ty)) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

int_id!(
    /// A pull request number (`pullRequestId`).
    PrNumber(i32)
);
int_id!(ThreadId(i32));
int_id!(CommentId(i32));
int_id!(
    /// Correlates async UI requests (people search) with their results.
    RequestId(u64)
);

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OrgIdError {
    #[error("empty organization")]
    Empty,
    #[error("invalid organization name: {0}")]
    Invalid(String),
    #[error("unsupported host: {0}")]
    UnsupportedHost(String),
    #[error("invalid URL: {0}")]
    Url(String),
}

/// A lowercase Azure DevOps organization name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrgId(Arc<str>);

impl OrgId {
    /// 1..=50 chars of `[a-z0-9-]`, first and last alphanumeric. Input is lowercased first.
    pub fn parse(name: &str) -> Result<Self, OrgIdError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(OrgIdError::Empty);
        }
        let s = trimmed.to_ascii_lowercase();
        let bytes = s.as_bytes();
        let valid = bytes.len() <= 50
            && bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
            && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
            && bytes.last().is_some_and(u8::is_ascii_alphanumeric);
        if valid {
            Ok(Self(Arc::from(s)))
        } else {
            Err(OrgIdError::Invalid(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for OrgId {
    type Error = OrgIdError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<OrgId> for String {
    fn from(o: OrgId) -> Self {
        o.0.as_ref().to_owned()
    }
}

impl FromStr for OrgId {
    type Err = OrgIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Accepts `contoso`, `dev.azure.com/contoso`, `https://dev.azure.com/contoso/Proj/_git/repo`,
/// `https://contoso.visualstudio.com[/DefaultCollection]` and `https://vssps.dev.azure.com/contoso`.
pub fn parse_org_input(input: &str) -> Result<OrgId, OrgIdError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(OrgIdError::Empty);
    }
    if !input.contains('/') && !input.contains('.') {
        return OrgId::parse(input);
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = Url::parse(&with_scheme).map_err(|e| OrgIdError::Url(e.to_string()))?;
    let host = url
        .host_str()
        .ok_or_else(|| OrgIdError::Url("missing host".into()))?
        .to_ascii_lowercase();
    if host == "dev.azure.com" || host == "vssps.dev.azure.com" {
        let first = url
            .path_segments()
            .and_then(|mut s| s.find(|seg| !seg.is_empty()))
            .ok_or(OrgIdError::Empty)?;
        return OrgId::parse(first);
    }
    if let Some(label) = host.strip_suffix(".visualstudio.com") {
        if label.is_empty() || label.contains('.') || label == "app" || label == "vssps" {
            return Err(OrgIdError::UnsupportedHost(host));
        }
        return OrgId::parse(label);
    }
    Err(OrgIdError::UnsupportedHost(host))
}

/// Uniquely identifies a pull request across orgs: `{org}/{repo}/{number}`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PrKey {
    pub org: OrgId,
    pub repo: RepoId,
    pub number: PrNumber,
}

impl PrKey {
    pub fn new(org: OrgId, repo: RepoId, number: PrNumber) -> Self {
        Self { org, repo, number }
    }

    /// Toast tag: `pr-` + 16 lowercase hex of fnv64(display). 19 chars, within the 64-char limit.
    pub fn tag(&self) -> String {
        format!("pr-{:016x}", fnv64(self.to_string().as_bytes()))
    }
}

impl fmt::Display for PrKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.org, self.repo, self.number)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid pull request key: {0}")]
pub struct PrKeyError(pub String);

impl FromStr for PrKey {
    type Err = PrKeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || PrKeyError(s.to_owned());
        let mut parts = s.splitn(3, '/');
        let org = parts.next().ok_or_else(err)?;
        let repo = parts.next().ok_or_else(err)?;
        let number = parts.next().ok_or_else(err)?;
        Ok(Self {
            org: OrgId::parse(org).map_err(|_| err())?,
            repo: repo.parse().map_err(|_| err())?,
            number: PrNumber(number.parse().map_err(|_| err())?),
        })
    }
}

impl TryFrom<String> for PrKey {
    type Error = PrKeyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<PrKey> for String {
    fn from(k: PrKey) -> Self {
        k.to_string()
    }
}

#[cfg(test)]
mod tests;
