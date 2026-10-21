//! A single-flight token cache: concurrent callers share one acquisition, and a recent
//! failure is reused briefly instead of spawning `az` again.

use std::{future::Future, sync::Arc, time::Duration};

use prm_domain::{
    Timestamp,
    auth::{AccessToken, AuthError},
    ports::Clock,
    secret::TokenId,
};

#[derive(Debug)]
enum Slot {
    Empty,
    Token(AccessToken),
    Failed { error: AuthError, at: Timestamp },
}

pub struct TokenCache {
    /// A tokio mutex held across the acquisition: that is what makes it single-flight.
    slot: tokio::sync::Mutex<Slot>,
    clock: Arc<dyn Clock>,
    skew: Duration,
    negative_ttl: Duration,
}

impl TokenCache {
    pub fn new(clock: Arc<dyn Clock>, skew: Duration, negative_ttl: Duration) -> Self {
        Self {
            slot: tokio::sync::Mutex::new(Slot::Empty),
            clock,
            skew,
            negative_ttl,
        }
    }

    /// Returns the cached token unless it is near expiry or is the `rejected` one; otherwise
    /// runs `acquire` once while other callers wait.
    pub async fn get<F, Fut>(
        &self,
        rejected: Option<TokenId>,
        acquire: F,
    ) -> Result<AccessToken, AuthError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<AccessToken, AuthError>> + Send,
    {
        let mut slot = self.slot.lock().await;
        let now = self.clock.now();
        match &*slot {
            Slot::Token(token) if rejected != Some(token.id) && token.valid_for(now, self.skew) => {
                return Ok(token.clone());
            }
            // A stamp in the future (the clock went backwards) counts as expired.
            Slot::Failed { error, at }
                if rejected.is_none() && *at <= now && now.since(*at) < self.negative_ttl =>
            {
                return Err(error.clone());
            }
            Slot::Empty | Slot::Token(_) | Slot::Failed { .. } => {}
        }
        let result = acquire().await;
        *slot = match &result {
            Ok(token) => Slot::Token(token.clone()),
            // why: stamp when the acquisition ended; a slow `az` failure (60 s) would otherwise
            // be cached already expired and every queued caller would spawn `az` again.
            Err(error) => Slot::Failed {
                error: error.clone(),
                at: self.clock.now(),
            },
        };
        result
    }

    /// Stores a token obtained interactively.
    pub async fn prime(&self, token: AccessToken) {
        *self.slot.lock().await = Slot::Token(token);
    }

    pub async fn invalidate(&self) {
        *self.slot.lock().await = Slot::Empty;
    }
}

#[cfg(test)]
mod tests;
