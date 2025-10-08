use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use std::time::Duration;
#[cfg(not(test))]
use std::time::SystemTime;

use chrono::DateTime;
#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use tokio::task;

use crate::reqwest::Response;
use crate::reqwest_retry::{DefaultRetryableStrategy, Retryable, RetryableStrategy};

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

pub struct DefaultRetryableStrategyInner(DefaultRetryableStrategy);

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
    fn handle(
        &self,
        res: &Result<Response, crate::reqwest_middleware::Error>,
    ) -> Option<Retryable> {
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
        let date: std::time::SystemTime = date.into();
        #[allow(clippy::useless_conversion)]
        return Some(date.into());
    }
    None
}

//noinspection DuplicatedCode
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use anyhow::anyhow;
    use rstest::rstest;

    use super::*;
    use crate::http::StatusCode;

    mod default_retry_strategy_inner {
        use super::*;

        mod impl_debug {
            use super::*;

            #[test]
            fn not_empty() {
                let strategy = DefaultRetryableStrategyInner::default();
                let debug = format!("{strategy:?}");
                assert!(!debug.is_empty());
            }
        }

        mod impl_clone {
            use super::*;

            #[test]
            fn all() {
                let strategy = DefaultRetryableStrategyInner::default();
                let strategy = strategy.clone();
                let response: Response = http::Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body("")
                    .unwrap()
                    .into();

                let actual = strategy.handle(&Ok(response));
                assert!(actual.is_some_and(|retryable| retryable == Retryable::Transient));
            }
        }

        mod impl_retryable_strategy {
            use super::*;

            #[rstest]
            #[case::ok(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::OK)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                None,
            )]
            #[case::bad_request(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                Some(Retryable::Fatal),
            )]
            #[case::too_many_requests(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                Some(Retryable::Transient),
            )]
            #[case::internal_server_error(
                {
                    let response: Response = http::Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body("")
                        .unwrap()
                        .into();
                    Ok(response)
                },
                Some(Retryable::Transient),
            )]
            #[case::middleware_error(
                Err(crate::reqwest_middleware::Error::Middleware(anyhow!("middleware error"))),
                Some(Retryable::Fatal),
            )]
            fn with(
                #[case] res: Result<Response, crate::reqwest_middleware::Error>,
                #[case] expected: Option<Retryable>,
            ) {
                let strategy = DefaultRetryableStrategyInner::default();
                let actual = strategy.handle(&res);
                // `Retryable` doesn't implement Debug so we can't use `assert_eq!`
                assert!(expected == actual);
            }
        }
    }

    mod parse_retry_after {
        use super::*;

        #[rstest]
        #[case::non_negative_integer("42", Some(SystemTime::now() + Duration::from_secs(42)))]
        #[case::negative_integer("-42", None)]
        #[case::zero("0", Some(SystemTime::now()))]
        #[case::rfc2822_date(
            "Wed, 21 Oct 2015 07:28:00 GMT",
            {
                let expected: std::time::SystemTime =
                    DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT").unwrap().into();
                Some(expected.into())
            },
        )]
        #[case::invalid_rfc2822_date("Foo, 21 Oct 2015 07:28:00 GMT", None)]
        #[case::invalid_string("The quick brown fox jumped over the lazy dog", None)]
        #[case::empty_string("", None)]
        fn with(#[case] val: &str, #[case] expected: Option<SystemTime>) {
            let actual = parse_retry_after(val);
            assert_eq!(expected, actual);
        }
    }
}
