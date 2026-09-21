//! Retry policy implementations

pub(crate) mod detail;

use std::sync::Arc;
#[cfg(not(test))]
use std::time::SystemTime;

#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use tokio::task;
use tracing::{debug, info, trace};

use crate::header::RetryAfterHeaderValue;
use crate::policy::detail::{RetryAfterPolicyInner, retryable_str};
use crate::reqwest::Response;
use crate::reqwest_retry::policies::ExponentialBackoff;
use crate::reqwest_retry::{
    DefaultRetryableStrategy, RetryDecision, RetryPolicy, Retryable, RetryableStrategy,
};

/// [`RetryPolicy`] that checks for HTTP headers indicating when to retry a request and uses
/// their values to determine the time between retries.
///
/// # Goal
///
/// This retry policy is designed to be used with the helpers from the [`reqwest_retry`] crate. When
/// a request needs to be retried, this policy will look for an HTTP header indicating when to retry
/// in the response and if found, will use its value. Such headers include the standard
/// [`RETRY_AFTER`] as well as [`X_RATELIMIT_RESET`].
///
/// Because of the way that [`RetryTransientMiddleware`] is designed, this policy implements _both_
/// [`RetryPolicy`] and [`RetryableStrategy`]. The decision on whether to retry a request, or how
/// many times to do so, is delegated to another combo of [`RetryPolicy`] / [`RetryableStrategy`].
/// The only thing this policy changes is that _if_ a request is retried _and_ a valid retry-after
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
/// concurrently outside Tokio tasks, their retry-after header values might get mixed up.
///
/// [`RETRY_AFTER`]: crate::http::header::RETRY_AFTER
/// [`X_RATELIMIT_RESET`]: crate::header::X_RATELIMIT_RESET
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

    #[cfg_attr(not(coverage), tracing::instrument(skip_all, fields(task_id = ?task::try_id()), level = "trace", ret))]
    pub(crate) fn get_retry_at(&self) -> Option<SystemTime> {
        self.0
            .retry_at
            .read()
            .unwrap()
            .get(&task::try_id())
            .copied()
    }

    #[cfg_attr(not(coverage), tracing::instrument(skip_all, fields(task_id = ?task::try_id()), level = "trace"))]
    pub(crate) fn set_retry_at(&self, retry_at: Option<SystemTime>) {
        trace!(?retry_at);

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
    #[cfg_attr(not(coverage), tracing::instrument(
        skip_all,
        fields(
            url = res.as_ref().map(|r| r.url().to_string()).ok(),
            status_code = res.as_ref().map(|r| r.status().to_string()).ok(),
            err = ?res.as_ref().err(),
        )
    ))]
    fn handle(&self, res: &Result<Response, reqwest_middleware::Error>) -> Option<Retryable> {
        info!(res_ok = res.is_ok());
        trace!(?res);

        let retryable = self.0.inner_strategy.handle(res);

        if retryable == Some(Retryable::Transient)
            && let Ok(response) = res
            && let Some(retry_after) = RetryAfterHeaderValue::from_response(response)
            && let Some(sleep_time) = retry_after.into_sleep_time(response)
        {
            let retry_at = SystemTime::now().checked_add(sleep_time);

            debug!(retry_after_header = ?retry_after, retry_sleep_time = ?sleep_time, ?retry_at);
            self.set_retry_at(retry_at);
        } else {
            debug!(retry_at = ?None::<SystemTime>);
            self.set_retry_at(None);
        }

        info!(ret = retryable.as_ref().map(retryable_str));
        retryable
    }
}

impl<P, S> RetryPolicy for RetryAfterPolicy<P, S>
where
    P: RetryPolicy,
{
    #[cfg_attr(not(coverage), tracing::instrument(skip_all, ret))]
    fn should_retry(
        &self,
        request_start_time: std::time::SystemTime,
        n_past_retries: u32,
    ) -> RetryDecision {
        info!(?request_start_time, n_past_retries);

        let decision = self
            .0
            .inner_policy
            .should_retry(request_start_time, n_past_retries);
        debug!(inner_decision = ?decision);

        if let RetryDecision::Retry { execute_after } = decision
            && let Some(retry_at) = self.get_retry_at()
        {
            debug!(
                inner_execute_after = ?execute_after,
                overriden_execute_after = ?retry_at,
                "overriding execute_after"
            );

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
    use reqwest_retry::Jitter;
    use rstest::rstest;
    use tokio::task::spawn_blocking;
    use tracing_test::traced_test;

    use super::*;
    use crate::http::StatusCode;
    use crate::http::header::RETRY_AFTER;

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

        #[rstest]
        #[case::with_policy_and_strategy(
            RetryAfterPolicy::with_policy_and_strategy(
                ExponentialBackoff::builder().build_with_max_retries(5),
                DefaultRetryableStrategy,
            )
        )]
        #[case::with_policy(
            RetryAfterPolicy::with_policy(
                ExponentialBackoff::builder().build_with_max_retries(5),
            )
        )]
        #[case::with_max_retries_and_strategy(RetryAfterPolicy::with_max_retries_and_strategy(
            5,
            DefaultRetryableStrategy
        ))]
        #[case::with_max_retries(RetryAfterPolicy::with_max_retries(5))]
        #[case::default(
            RetryAfterPolicy::<UselessPolicy, UselessPolicy>::default()
        )]
        #[tokio::test]
        #[traced_test]
        async fn with<P, S>(#[case] policy: RetryAfterPolicy<P, S>)
        where
            P: 'static,
            S: 'static,
            RetryAfterPolicy<P, S>: Clone + Send + Sync,
        {
            let retry_at = SystemTime::now();
            policy.set_retry_at(Some(retry_at));
            let actual = policy.get_retry_at();
            assert_eq!(actual, Some(retry_at));

            {
                let policy = policy.clone();
                spawn_blocking(|| async move {
                    let retry_at = SystemTime::now() + Duration::from_secs(7);
                    policy.set_retry_at(Some(retry_at));
                    let actual = policy.get_retry_at();
                    assert_eq!(actual, Some(retry_at));
                });
            }

            let actual = policy.get_retry_at();
            assert_eq!(actual, Some(retry_at));

            spawn_blocking(|| async move {
                let actual = policy.get_retry_at();
                assert!(actual.is_none());
            });
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
            #[traced_test]
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
