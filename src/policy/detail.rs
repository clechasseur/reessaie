use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use std::time::Duration;
#[cfg(not(test))]
use std::time::SystemTime;

use chrono::DateTime;
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

//noinspection DuplicatedCode
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use rstest::rstest;

    use super::*;

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
