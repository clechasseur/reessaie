//! Retry policy implementations

pub(crate) mod detail;

use std::sync::Arc;
#[cfg(not(test))]
use std::time::SystemTime;

#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use reqwest::Response;
use reqwest::header::RETRY_AFTER;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::{
    DefaultRetryableStrategy, RetryDecision, RetryPolicy, Retryable, RetryableStrategy,
};
use tokio::task;

use crate::policy::detail::{RetryAfterPolicyInner, parse_retry_after};

/// [`RetryPolicy`] that checks for the [`Retry-After`] HTTP header and uses its value to
/// determine the time between retries.
///
/// # Goal
///
/// This retry policy is designed to be used with the helpers from the [`reqwest_retry`] crate. When
/// a request needs to be retried, this policy will look for a [`Retry-After`] HTTP header in the
/// response and if found, will use that value to determine when to retry.
///
/// Because of the way that [`RetryTransientMiddleware`] is designed, this policy implements _both_
/// [`RetryPolicy`] and [`RetryableStrategy`]. The decision on whether to retry a request, or how
/// many times to do so, is delegated to another combo of [`RetryPolicy`] / [`RetryableStrategy`].
/// The only thing this policy changes is that _if_ a request is retried _and_ a valid [`Retry-After`]
/// HTTP header is present in the response, _then_ the value of that header is used to determine
/// how long to wait before retrying; otherwise, the wait time determined by the inner policy is used.
///
/// # Usage
///
/// Because this policy must be used both as the [`RetryPolicy`] and the [`RetryableStrategy`]
/// in the middleware, which is a bit unintuitive, a custom middleware is available:
/// [`RetryAfterMiddleware`]; see its documentation for details.
///
/// # `RetryAfterPolicy` and `tokio`
///
/// Because of the way this policy is implemented, it needs to be able to uniquely identify the
/// task currently executing when a request is performed. Currently, this is only possible when
/// using the [`tokio`] runtime (through [`try_id`]).
///
/// This policy can still be used outside a Tokio task, but if more than one request are performed
/// concurrently outside of Tokio tasks, their `Retry-After` header values might get mixed up.
///
/// [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
/// [`RetryTransientMiddleware`]: reqwest_retry::RetryTransientMiddleware
/// [`RetryAfterMiddleware`]: crate::RetryAfterMiddleware
/// [`try_id`]: task::try_id
#[derive(Debug)]
pub struct RetryAfterPolicy<P = ExponentialBackoff, S = DefaultRetryableStrategy>(
    Arc<RetryAfterPolicyInner<P, S>>,
);

impl<P, S> RetryAfterPolicy<P, S> {
    /// Creates a new [`RetryAfterPolicy`] wrapping the given inner policy and strategy.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use reessaie::RetryAfterPolicy;
    /// # use reessaie::reqwest_retry::DefaultRetryableStrategy;
    /// # use reessaie::reqwest_retry::policies::ExponentialBackoff;
    ///
    /// let policy = RetryAfterPolicy::with_policy_and_strategy(
    ///     ExponentialBackoff::builder().build_with_total_retry_duration(Duration::from_secs(30)),
    ///     DefaultRetryableStrategy,
    /// );
    /// ```
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

impl<P> RetryAfterPolicy<P, DefaultRetryableStrategy> {
    /// Creates a new [`RetryAfterPolicy`] wrapping the given inner policy.
    /// This will automatically use the [`DefaultRetryableStrategy`].
    ///
    /// # Example
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use reqwest_retry::policies::ExponentialBackoff;
    /// # use reessaie::RetryAfterPolicy;
    ///
    /// let policy = RetryAfterPolicy::with_policy(
    ///     ExponentialBackoff::builder().build_with_total_retry_duration(Duration::from_secs(30)),
    /// );
    /// ```
    pub fn with_policy(inner_policy: P) -> Self {
        Self::with_policy_and_strategy(inner_policy, DefaultRetryableStrategy)
    }
}

impl<S> RetryAfterPolicy<ExponentialBackoff, S> {
    /// Creates a new [`RetryAfterPolicy`] wrapping an inner [`ExponentialBackoff`]
    /// policy that will retry the given number of times, and the given strategy.
    ///
    /// # Example
    ///
    /// ```
    /// # use reessaie::RetryAfterPolicy;
    /// # use reqwest_retry::DefaultRetryableStrategy;
    ///
    /// let policy = RetryAfterPolicy::with_max_retries_and_strategy(5, DefaultRetryableStrategy);
    /// ```
    pub fn with_max_retries_and_strategy(max_retries: u32, strategy: S) -> Self {
        Self::with_policy_and_strategy(
            ExponentialBackoff::builder().build_with_max_retries(max_retries),
            strategy,
        )
    }
}

impl RetryAfterPolicy<ExponentialBackoff, DefaultRetryableStrategy> {
    /// Creates a new [`RetryAfterPolicy`] wrapping an inner [`ExponentialBackoff`]
    /// policy that will retry the given number of times. This will automatically use
    /// the [`DefaultRetryableStrategy`].
    ///
    /// # Example
    ///
    /// ```
    /// # use reessaie::RetryAfterPolicy;
    ///
    /// let policy = RetryAfterPolicy::with_max_retries(5);
    /// ```
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self::with_max_retries_and_strategy(max_retries, DefaultRetryableStrategy)
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

impl<P, S> Clone for RetryAfterPolicy<P, S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
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
        {
            self.set_retry_at(parse_retry_after(retry_after));
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
    use http::StatusCode;
    use reqwest_retry::Jitter;
    use rstest::rstest;
    use tokio::task::spawn_blocking;

    use super::*;

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
                DefaultRetryableStrategy,
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
            let policy =
                RetryAfterPolicy::with_max_retries_and_strategy(5, DefaultRetryableStrategy);
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn with_max_retries() {
            let policy = RetryAfterPolicy::with_max_retries(5);
            test_policy(policy).await;
        }

        #[tokio::test]
        async fn default() {
            let policy: RetryAfterPolicy<UselessPolicy, UselessPolicy> = <_>::default();
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
                Err(reqwest_middleware::Error::Middleware(anyhow!("middleware error"))),
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
