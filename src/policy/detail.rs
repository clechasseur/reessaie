use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
#[cfg(not(test))]
use std::time::SystemTime;

#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use tokio::task;

use crate::reqwest_retry::Retryable;

#[derive(Debug)]
pub struct RetryAfterPolicyInner<P, S> {
    pub inner_policy: P,
    pub inner_strategy: S,
    pub retry_at: RwLock<HashMap<Option<task::Id>, SystemTime>>,
}

impl<P, S> RetryAfterPolicyInner<P, S> {
    pub fn new(inner_policy: P, inner_strategy: S) -> Arc<Self> {
        Arc::new(Self { inner_policy, inner_strategy, retry_at: RwLock::new(HashMap::new()) })
    }
}

/// Returns a string representation of a [`Retryable`] value.
///
/// This is required because [`Retryable`] implements neither `Debug` nor `Display`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn retryable_str(retryable: &Retryable) -> &'static str {
    match retryable {
        Retryable::Transient => "Retryable::Transient",
        Retryable::Fatal => "Retryable::Fatal",
    }
}
