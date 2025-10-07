use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use chrono::DateTime;
use tokio::task;
use crate::reqwest::Response;
use crate::reqwest_middleware::Error;
use crate::reqwest_retry::{DefaultRetryableStrategy, Retryable, RetryableStrategy};

#[derive(Debug)]
pub struct RetryAfterPolicyInner<P, S> {
    pub inner_policy: P,
    pub inner_strategy: S,
    pub retry_at: RwLock<HashMap<task::Id, SystemTime>>,
}

impl<P, S> RetryAfterPolicyInner<P, S> {
    pub fn new(inner_policy: P, inner_strategy: S) -> Arc<Self> {
        Arc::new(Self {
            inner_policy,
            inner_strategy,
            retry_at: RwLock::new(HashMap::new()),
        })
    }
}

pub struct DefaultRetryableStrategyInner(DefaultRetryableStrategy);

impl DefaultRetryableStrategyInner {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Debug for DefaultRetryableStrategyInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultRetryableStrategyInner").finish()
    }
}

impl Clone for DefaultRetryableStrategyInner {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Default for DefaultRetryableStrategyInner {
    fn default() -> Self {
        Self(DefaultRetryableStrategy)
    }
}

impl RetryableStrategy for DefaultRetryableStrategyInner {
    fn handle(&self, res: &Result<Response, Error>) -> Option<Retryable> {
        self.0.handle(res)
    }
}

/// Parses the content of a `Retry-After` header.
///
/// Copied from [`reqwest-retry-after`] (see [here]) and adapted.
///
/// [`reqwest-retry-after`]: https://crates.io/crates/reqwest-retry-after
/// [here]: https://github.com/melotic/reqwest-retry-after/blob/d80bf48b434a70998191ad01d06d58e77b931b2f/src/lib.rs#L56-L64
pub fn parse_retry_after(val: &str) -> Option<SystemTime> {
    if let Ok(secs) = val.parse::<u64>() {
        return Some(SystemTime::now() + Duration::from_secs(secs));
    }
    if let Ok(date) = DateTime::parse_from_rfc2822(val) {
        return Some(date.into());
    }
    None
}
