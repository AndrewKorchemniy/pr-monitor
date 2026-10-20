//! `prmonitor://` URIs, used by toast activation and the single-instance pipe.
//! Parsing is a strict allow-list: anything unexpected is rejected.

use std::{fmt, str::FromStr};

use crate::{
    app::URI_SCHEME,
    ids::{OrgId, PrKey, PrNumber, RepoId},
};

/// Longest URI accepted, in bytes.
pub const MAX_URI_LEN: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppUri {
    /// Bring the window forward, optionally highlighting one PR.
    Show { key: Option<PrKey> },
    /// Refresh every org now.
    Refresh,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AppUriError {
    #[error("URI is longer than {MAX_URI_LEN} bytes")]
    TooLong,
    #[error("URI does not use the {URI_SCHEME}:// scheme")]
    Scheme,
    #[error("unknown action: {0}")]
    Host(String),
    #[error("unknown query key: {0}")]
    Key(String),
    #[error("org, repo and pr must be given together")]
    PartialKey,
    #[error("invalid value: {0}")]
    Invalid(String),
}

impl AppUri {
    pub fn parse(s: &str) -> Result<Self, AppUriError> {
        if s.len() > MAX_URI_LEN {
            return Err(AppUriError::TooLong);
        }
        let url = url::Url::parse(s.trim()).map_err(|e| AppUriError::Invalid(e.to_string()))?;
        if !url.scheme().eq_ignore_ascii_case(URI_SCHEME) {
            return Err(AppUriError::Scheme);
        }
        let action = url.host_str().unwrap_or_default().to_ascii_lowercase();
        match action.as_str() {
            "show" => Ok(Self::Show {
                key: parse_key(&url)?,
            }),
            "refresh" if url.query_pairs().next().is_none() => Ok(Self::Refresh),
            "refresh" => Err(AppUriError::Key("refresh takes no parameters".into())),
            other => Err(AppUriError::Host(other.to_owned())),
        }
    }
}

/// Reads the optional `org`, `repo`, `pr` triple; all three or none.
fn parse_key(url: &url::Url) -> Result<Option<PrKey>, AppUriError> {
    let (mut org, mut repo, mut pr) = (None, None, None);
    for (name, value) in url.query_pairs() {
        let slot = match name.as_ref() {
            "org" => &mut org,
            "repo" => &mut repo,
            "pr" => &mut pr,
            other => return Err(AppUriError::Key(other.to_owned())),
        };
        *slot = Some(value.into_owned());
    }
    let (org, repo, pr) = match (org, repo, pr) {
        (None, None, None) => return Ok(None),
        (Some(org), Some(repo), Some(pr)) => (org, repo, pr),
        _ => return Err(AppUriError::PartialKey),
    };
    let invalid = |what: &str| AppUriError::Invalid(what.to_owned());
    Ok(Some(PrKey::new(
        OrgId::parse(&org).map_err(|_| invalid("org"))?,
        RepoId::from_str(&repo).map_err(|_| invalid("repo"))?,
        PrNumber(pr.parse().map_err(|_| invalid("pr"))?),
    )))
}

impl fmt::Display for AppUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Show { key: None } => write!(f, "{URI_SCHEME}://show"),
            Self::Show { key: Some(k) } => write!(
                f,
                "{URI_SCHEME}://show?org={}&repo={}&pr={}",
                k.org, k.repo, k.number
            ),
            Self::Refresh => write!(f, "{URI_SCHEME}://refresh"),
        }
    }
}

#[cfg(test)]
mod tests;
