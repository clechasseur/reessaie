//! Retry policy implementations

pub(crate) mod detail;

use std::sync::Arc;
#[cfg(not(test))]
use std::time::SystemTime;

#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use tokio::task;

use crate::policy::detail::{
    DefaultRetryableStrategyInner, RetryAfterPolicyInner, parse_retry_after,
};
use crate::reqwest::Response;
use crate::reqwest::header::RETRY_AFTER;
use crate::reqwest_retry::policies::ExponentialBackoff;
use crate::reqwest_retry::{RetryDecision, RetryPolicy, Retryable, RetryableStrategy};

/// [`RetryPolicy`] that checks for the [`Retry-After`] HTTP header and uses its value to
/// determine the time between retries. If not available, forwards the decision to another policy.
///
/// _TODO add more details_
///
/// [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
#[derive(Debug, Clone)]
pub struct RetryAfterPolicy<P = ExponentialBackoff, S = DefaultRetryableStrategyInner>(
    Arc<RetryAfterPolicyInner<P, S>>,
);

impl<P, S> RetryAfterPolicy<P, S> {
    /// _TODO_
    pub fn with_policy_and_strategy(inner_policy: P, inner_strategy: S) -> Self {
        Self(RetryAfterPolicyInner::new(inner_policy, inner_strategy))
    }

    pub(crate) fn get_retry_at(&self) -> Option<SystemTime> {
        self.0
            .retry_at
            .read()
            .unwrap()
            .get(&task::try_id())
            .copied()
    }

    pub(crate) fn set_retry_at(&self, retry_at: Option<SystemTime>) {
        let task_id = task::try_id();
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

impl<P> RetryAfterPolicy<P, DefaultRetryableStrategyInner> {
    /// _TODO_
    pub fn with_policy(inner_policy: P) -> Self {
        Self::with_policy_and_strategy(inner_policy, <_>::default())
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

impl RetryAfterPolicy<ExponentialBackoff, DefaultRetryableStrategyInner> {
    /// _TODO_
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self::with_max_retries_and_strategy(max_retries, <_>::default())
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
    fn handle(
        &self,
        res: &Result<Response, crate::reqwest_middleware::Error>,
    ) -> Option<Retryable> {
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
    fn should_retry(
        &self,
        request_start_time: std::time::SystemTime,
        n_past_retries: u32,
    ) -> RetryDecision {
        let decision = self
            .0
            .inner_policy
            .should_retry(request_start_time, n_past_retries);

        if let RetryDecision::Retry { execute_after: _ } = decision
            && let Some(retry_at) = self.get_retry_at()
        {
            #[allow(clippy::useless_conversion)]
            RetryDecision::Retry { execute_after: retry_at.into() }
        } else {
            decision
        }
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::time::Duration;

    use anyhow::anyhow;
    use rstest::rstest;
    use tokio::task::spawn_blocking;

    use super::*;
    use crate::http::StatusCode;
    use crate::reqwest_retry::Jitter;

    mod retry_after_policy {
        use super::*;

        #[derive(Debug, Copy, Clone, Default)]
        struct UselessPolicy;

        impl RetryPolicy for UselessPolicy {
            fn should_retry(
                &self,
                _request_start_time: std::time::SystemTime,
                _n_past_retries: u32,
            ) -> RetryDecision {
                RetryDecision::DoNotRetry
            }
        }

        async fn test_policy<P, S>(policy: RetryAfterPolicy<P, S>)
        where
            P: 'static,
            S: 'static,
            RetryAfterPolicy<P, S>: Clone + Send + Sync,
        {
            policy.set_retry_at(Some(SystemTime::now()));
            let actual = policy.get_retry_at();
            assert_eq!(actual, Some(SystemTime::now()));

            {
                let policy = policy.clone();
                spawn_blocking(|| async move {
                    policy.set_retry_at(Some(SystemTime::now()));
                    let actual = policy.get_retry_at();
                    assert_eq!(actual, Some(SystemTime::now()));
                });
            }

            spawn_blocking(|| async move {
                let actual = policy.get_retry_at();
                assert!(actual.is_none());
            });
        }

        #[tokio::test]
        async fn with_policy_and_strategy() {
            let policy = RetryAfterPolicy::with_policy_and_strategy(
                ExponentialBackoff::builder().build_with_max_retries(5),
                DefaultRetryableStrategyInner::default(),
            );
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn with_policy() {
            let policy = RetryAfterPolicy::with_policy(
                ExponentialBackoff::builder().build_with_max_retries(5),
            );
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn with_max_retries_and_strategy() {
            let policy = RetryAfterPolicy::with_max_retries_and_strategy(
                5,
                DefaultRetryableStrategyInner::default(),
            );
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn with_max_retries() {
            let policy = RetryAfterPolicy::with_max_retries(5);
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn default() {
            let policy: RetryAfterPolicy<UselessPolicy, DefaultRetryableStrategyInner> =
                <_>::default();
            test_policy(policy).await;
        }

        mod impl_retryable_strategy_and_retry_policy {
            use super::*;

            #[rstest]
            #[case::not_retryable(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::OK)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                None,
                None,
            )]
            #[case::failed_request(
                Err(crate::reqwest_middleware::Error::Middleware(anyhow!("middleware error"))),
                Some(Retryable::Fatal),
                None,
            )]
            #[case::transient_retryable_without_retry_after_header(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                Some(Retryable::Transient),
                Some(std::time::SystemTime::now() + Duration::from_secs(10)),
            )]
            #[case::transient_retryable_with_retry_after_header(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .header(RETRY_AFTER, "23")
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                Some(Retryable::Transient),
                Some((SystemTime::now() + Duration::from_secs(23)).into()),
            )]
            fn with(
                #[case] res: Result<Response, crate::reqwest_middleware::Error>,
                #[case] expected_retryable: Option<Retryable>,
                #[case] expected_execute_after: Option<std::time::SystemTime>,
            ) {
                let current_time = SystemTime::now();
                let policy = RetryAfterPolicy::with_policy(
                    ExponentialBackoff::builder()
                        .retry_bounds(Duration::from_secs(10), Duration::from_secs(10))
                        .jitter(Jitter::None)
                        .build_with_max_retries(5),
                );

                let actual_retryable = policy.handle(&res);
                // `Retryable` doesn't implement Debug so we can't use `assert_eq!`
                assert!(actual_retryable == expected_retryable);

                if expected_retryable == Some(Retryable::Transient) {
                    let actual_decision = policy.should_retry(current_time.into(), 0);
                    match (expected_execute_after, actual_decision) {
                        (
                            Some(expected_execute_after),
                            RetryDecision::Retry { execute_after: actual_execute_after },
                        ) => {
                            let diff = if expected_execute_after < actual_execute_after {
                                actual_execute_after
                                    .duration_since(expected_execute_after)
                                    .unwrap()
                            } else {
                                expected_execute_after
                                    .duration_since(actual_execute_after)
                                    .unwrap()
                            };
                            assert!(diff < Duration::from_secs(1));
                        },
                        (None, RetryDecision::DoNotRetry) => (),
                        (expected_wait_time, actual_decision) => {
                            panic!(
                                "mismatched decisions: expected to wait {expected_wait_time:?}, got {actual_decision:?}"
                            );
                        },
                    }
                }
            }
        }
    }
}
