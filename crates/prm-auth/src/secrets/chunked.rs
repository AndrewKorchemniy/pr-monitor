//! Splits large values across several slots. Small values are stored raw. A large value is
//! stored as parts `key#0..n` plus a header in `key`: `PRMC1:{n}:{len}:{fnv64}`. The header is
//! deleted before any part is rewritten and written last, so a failed or interrupted write
//! reads as absent, never as a header pointing at missing or mixed parts.

use prm_domain::{hash::fnv64, secret::SecretBytes};

use super::{SecretKey, SecretStore, SecretStoreError};
use crate::consts::{CHUNK_MAGIC, CHUNK_SIZE};

#[derive(Debug)]
pub struct ChunkedSecretStore<S: SecretStore> {
    inner: S,
}

/// A parsed chunk header.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Header {
    parts: usize,
    len: usize,
    checksum: u64,
}

impl Header {
    fn encode(self) -> String {
        format!(
            "{CHUNK_MAGIC}{}:{}:{:016x}",
            self.parts, self.len, self.checksum
        )
    }

    /// `None` when the value is not a header (a raw small value).
    fn parse(value: &[u8]) -> Option<Result<Self, SecretStoreError>> {
        let text = std::str::from_utf8(value).ok()?.strip_prefix(CHUNK_MAGIC)?;
        let corrupt = || SecretStoreError::Corrupt("bad chunk header".into());
        let fields: Vec<&str> = text.split(':').collect();
        let [parts, len, checksum] = fields.as_slice() else {
            return Some(Err(corrupt()));
        };
        Some(Self::from_fields(parts, len, checksum).ok_or_else(corrupt))
    }

    fn from_fields(parts: &str, len: &str, checksum: &str) -> Option<Self> {
        Some(Self {
            parts: parts.parse().ok()?,
            len: len.parse().ok()?,
            checksum: u64::from_str_radix(checksum, 16).ok()?,
        })
    }
}

impl<S: SecretStore> ChunkedSecretStore<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    /// The header currently stored under `key`, if any (ignores unreadable values).
    fn current_header(&self, key: &SecretKey) -> Option<Header> {
        let value = self.inner.get(key).ok()??;
        Header::parse(&value)?.ok()
    }

    /// Deletes parts `from..` of an old value. Best effort: stale parts are harmless.
    fn delete_parts(&self, key: &SecretKey, old: Option<Header>, from: usize) {
        let Some(old) = old else { return };
        for index in from..old.parts {
            if let Err(e) = self.inner.delete(&key.part(index)) {
                tracing::warn!(error = %e, "could not delete a stale secret part");
            }
        }
    }

    fn read_parts(&self, key: &SecretKey, header: Header) -> Result<SecretBytes, SecretStoreError> {
        // why: the header is untrusted input; never allocate more than its parts can hold.
        let mut value = Vec::with_capacity(header.len.min(header.parts.saturating_mul(CHUNK_SIZE)));
        for index in 0..header.parts {
            let part = self
                .inner
                .get(&key.part(index))?
                .ok_or_else(|| SecretStoreError::Corrupt(format!("missing part {index}").into()))?;
            value.extend_from_slice(&part);
        }
        if value.len() != header.len || fnv64(&value) != header.checksum {
            return Err(SecretStoreError::Corrupt("checksum mismatch".into()));
        }
        Ok(SecretBytes::new(value))
    }
}

impl<S: SecretStore> SecretStore for ChunkedSecretStore<S> {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretBytes>, SecretStoreError> {
        let Some(value) = self.inner.get(key)? else {
            return Ok(None);
        };
        match Header::parse(&value) {
            None => Ok(Some(value)),
            Some(header) => self.read_parts(key, header?).map(Some),
        }
    }

    fn set(&self, key: &SecretKey, value: &[u8]) -> Result<(), SecretStoreError> {
        let old = self.current_header(key);
        if value.len() <= CHUNK_SIZE {
            self.inner.set(key, value)?;
            self.delete_parts(key, old, 0);
            return Ok(());
        }
        if old.is_some() {
            // The old header would point at a mix of new and old parts if a write below fails.
            self.inner.delete(key)?;
        }
        let chunks: Vec<&[u8]> = value.chunks(CHUNK_SIZE).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            self.inner.set(&key.part(index), chunk)?;
        }
        let header = Header {
            parts: chunks.len(),
            len: value.len(),
            checksum: fnv64(value),
        };
        self.inner.set(key, header.encode().as_bytes())?;
        self.delete_parts(key, old, chunks.len());
        Ok(())
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretStoreError> {
        let old = self.current_header(key);
        self.inner.delete(key)?;
        self.delete_parts(key, old, 0);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
