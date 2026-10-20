//! Secrets never print, never serialize, and are zeroized on drop.

use std::fmt;

use zeroize::Zeroize;

use crate::hash::fnv64;

/// A token, PAT or pre-encoded auth header value. No `Display`, no `Serialize`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Box<str>);

impl Secret {
    /// Copies the text into an exact-size buffer, then zeroizes the source, so no spare-capacity
    /// copy is freed unzeroized (`into_boxed_str` may reallocate and drop the old buffer).
    pub fn new(s: impl Into<String>) -> Self {
        let mut source = s.into();
        let secret = Self(Box::from(source.as_str()));
        source.zeroize();
        secret
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A stable, log-safe identity for this secret value.
    pub fn id(&self) -> TokenId {
        TokenId(fnv64(self.0.as_bytes()))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub type SecretBytes = zeroize::Zeroizing<Vec<u8>>;

/// fnv64 of a secret; safe to log and compare.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TokenId(pub u64);

#[cfg(test)]
mod tests;
