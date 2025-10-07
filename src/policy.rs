//! Retry policy implementations

pub(crate) mod detail;

use std::sync::Arc;
use std::time::SystemTime;
use tokio::task;
use crate::policy::detail::{parse_retry_after, DefaultRetryableStrategyInner, RetryAfterPolicyInner};
use crate::reqwest::header::RETRY_AFTER;
use crate::reqwest::Response;
use crate::reqwest_retry::{RetryDecision, RetryPolicy, Retryable, RetryableStrategy};
use crate::reqwest_retry::policies::ExponentialBackoff;

/// [`RetryPolicy`] that checks for the [`Retry-After`] HTTP header and uses its value to
/// determine the time between retries. If not available, forwards the decision to another policy.
///
/// _TODO add more details_
///
/// [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
#[derive(Debug, Clone)]
pub struct RetryAfterPolicy<P = ExponentialBackoff, S = DefaultRetryableStrategyInner>(Arc<RetryAfterPolicyInner<P, S>>);

impl<P, S> RetryAfterPolicy<P, S> {
    /// _TODO_
    pub fn with_policy_and_strategy(inner_policy: P, inner_strategy: S) -> Self {
        Self(RetryAfterPolicyInner::new(
            inner_policy,
            inner_strategy,
        ))
    }

    pub(crate) fn get_retry_at(&self) -> Option<SystemTime> {
        task::try_id().and_then(|id| self.0.retry_at.read().unwrap().get(&id).copied())
    }

    pub(crate) fn set_retry_at(&self, retry_at: Option<SystemTime>) {
        if let Some(task_id) = task::try_id() {
            match retry_at {
                Some(retry_at) => {
                    self.0.retry_at.write().unwrap().insert(task_id, retry_at);
                },
                None => {
                    self.0.retry_at.write().unwrap().remove(&task_id);
                },
            }
        }
    }
}

impl<P, S> RetryAfterPolicy<P, S>
where
    S: Default,
{
    /// _TODO_
    pub fn with_policy(inner_policy: P) -> Self {
        Self::with_policy_and_strategy(inner_policy, S::default())
    }
}

impl<S> RetryAfterPolicy<ExponentialBackoff, S> {
    /// _TODO_
    pub fn with_max_retries_and_strategy(max_retries: u32, strategy: S) -> Self {
        Self::with_policy_and_strategy(
            ExponentialBackoff::builder().build_with_max_retries(max_retries),
            strategy,
        )
    }
}

impl<S> RetryAfterPolicy<ExponentialBackoff, S>
where
    S: Default,
{
    /// _TODO_
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self::with_max_retries_and_strategy(max_retries, S::default())
    }
}

impl<P, S> Default for RetryAfterPolicy<P, S>
where
    P: Default,
    S: Default,
{
    fn default() -> Self {
        Self::with_policy_and_strategy(P::default(), S::default())
    }
}

impl<P, S> RetryableStrategy for RetryAfterPolicy<P, S>
where
    S: RetryableStrategy,
{
    fn handle(&self, res: &Result<Response, reqwest_middleware::Error>) -> Option<Retryable> {
        let retryable = self.0.inner_strategy.handle(res);

        if let Some(Retryable::Transient) = retryable
            && let Ok(response) = res
            && let Some(retry_after) = response.headers().get(RETRY_AFTER)
            && let Ok(retry_after) = retry_after.to_str()
            && let Some(retry_at) = parse_retry_after(retry_after)
        {
            self.set_retry_at(Some(retry_at));
        } else {
            self.set_retry_at(None);
        }

        retryable
    }
}

impl<P, S> RetryPolicy for RetryAfterPolicy<P, S>
where
    P: RetryPolicy,
{
    fn should_retry(&self, request_start_time: SystemTime, n_past_retries: u32) -> RetryDecision {
        let decision = self.0.inner_policy.should_retry(request_start_time, n_past_retries);
        
        if let RetryDecision::Retry { execute_after: _ } = decision
            && let Some(retry_at) = self.get_retry_at()
        {
            RetryDecision::Retry { execute_after: retry_at }
        } else {
            decision
        }
    }
}
