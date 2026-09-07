//! Retry header utilities

use std::time::Duration;
#[cfg(not(test))]
use std::time::SystemTime;
use chrono::{DateTime, Utc};
use crate::http::header::{DATE, RETRY_AFTER};
use crate::http::HeaderValue;
#[cfg(test)]
use mock_instant::thread_local::SystemTime;
use crate::reqwest::Response;

/// Name of non-standard [`X-RateLimit-Reset`] HTTP header.
///
/// In conjunction with [`X-RateLimit-Limit`] and [`X-RateLimit-Remaining`], this
/// HTTP header is widely used to communicate how many requests are left in a
/// given rate limit window and when to retry when limits are hit.
///
/// [`X-RateLimit-Reset`]: https://http.dev/x-ratelimit-reset
/// [`X-RateLimit-Limit`]: https://http.dev/x-ratelimit-limit
/// [`X-RateLimit-Remaining`]: https://http.dev/x-ratelimit-remaining
pub const X_RATELIMIT_RESET: &str = "X-RateLimit-Reset";

/// Alternate name for the [`X-RateLimit-Reset`] HTTP header.
///
/// Some providers like Twitter/X use this variant instead of the [default one].
///
/// [`X-RateLimit-Reset`]: https://http.dev/x-ratelimit-reset
/// [default one]: X_RATELIMIT_RESET
pub const X_RATE_LIMIT_RESET: &str = "X-Rate-Limit-Reset";

/// Possible values that can be contained in HTTP headers indicating when to retry a request.
///
/// Such headers include the standard [`RETRY_AFTER`] as well as [`X_RATELIMIT_RESET`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RetryAfterHeaderValue {
    /// The header contains a timestamp indicating the earliest moment when the client can
    /// retry the request.
    ///
    /// The timestamp should be evaluated relative to the HTTP [`DATE`] header if present.
    Timestamp(DateTime<Utc>),

    /// The header contains a duration indicating how long to wait until the client can
    /// retry the request.
    ///
    /// The moment when to retry the request should be calculated by adding this duration
    /// to the timestamp in the HTTP [`DATE`] header if present.
    SleepTime(Duration),
}

impl RetryAfterHeaderValue {
    /// Looks for an HTTP response header indicating when to retry a request.
    ///
    /// Scans the headers in the given HTTP response, looking for those headers, in order:
    ///
    /// - [`Retry-After`]
    /// - [`X-RateLimit-Reset`]
    /// - [`X-RateLimit-Reset`] (an alternate spelling of the previous one)
    ///
    /// If one of those headers is found, and it contains data indicating when the client should
    /// retry a request, that information is returned in the form of a [`RetryAfterHeaderValue`](Self).
    /// Returns `None` otherwise.
    ///
    /// [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
    /// [`X-RateLimit-Reset`]: https://http.dev/x-ratelimit-reset
    /// [`X-RateLimit-Reset`]: https://http.dev/x-ratelimit-reset
    pub fn from_response(response: &Response) -> Option<Self> {
        let headers = response.headers();
        headers
            .get(RETRY_AFTER)
            .and_then(parse_retry_after_header)
            .or_else(|| headers.get(X_RATELIMIT_RESET).and_then(parse_x_rate_limit_reset_header))
            .or_else(|| headers.get(X_RATE_LIMIT_RESET).and_then(parse_x_rate_limit_reset_header))
    }

    /// Given this retry-after header value, computes how long the client must sleep before
    /// retrying the request.
    ///
    /// Uses the given HTTP response to find the [`DATE`] header if required.
    pub fn into_sleep_time(self, response: &Response) -> Option<Duration> {
        match self {
            Self::Timestamp(ts) => {
                // We should use the server's date/time as baseline if possible.
                let server_now = response
                    .headers()
                    .get(DATE)
                    .and_then(|val| val.to_str().ok())
                    .and_then(|val| DateTime::parse_from_rfc2822(val).ok())
                    .map(|d| d.to_utc().into())
                    .unwrap_or_else(|| {
                        let now = SystemTime::now();
                        #[allow(clippy::useless_conversion)]
                        now.into()
                    });

                std::time::SystemTime::from(ts).duration_since(server_now).ok()
            },
            Self::SleepTime(sleep_time) => Some(sleep_time),
        }
    }
}

/// Parses the content of a [`Retry-After`] HTTP header.
///
/// Copied from [`reqwest-retry-after`] (see [here]) and adapted.
///
/// [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
/// [`reqwest-retry-after`]: https://crates.io/crates/reqwest-retry-after
/// [here]: https://github.com/melotic/reqwest-retry-after/blob/d80bf48b434a70998191ad01d06d58e77b931b2f/src/lib.rs#L56-L64
pub fn parse_retry_after_header(val: &HeaderValue) -> Option<RetryAfterHeaderValue> {
    val
        .to_str()
        .ok()
        .and_then(|val| match val {
            val if let Ok(secs) = val.parse::<u64>() => {
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(secs)))
            },
            val if let Ok(date) = DateTime::parse_from_rfc2822(val) => {
                Some(RetryAfterHeaderValue::Timestamp(date.to_utc()))
            },
            _ => None,
        })
}

/// Parses the content of a [`X-RateLimit-Reset`] HTTP header.
///
/// [`X-RateLimit-Reset`]: https://http.dev/x-ratelimit-reset
pub fn parse_x_rate_limit_reset_header(val: &HeaderValue) -> Option<RetryAfterHeaderValue> {
    // X-RateLimit-Reset can contain either a Unix timestamp representing the
    // moment after which we can retry, or a number of seconds to wait. Unfortunately,
    // there is no standardized way to knowing which it is.
    // We'll use this heuristic: if server is telling us to wait for at least one day,
    // we'll assume it's a Unix timestamp.
    val
        .to_str()
        .ok()
        .and_then(|val| match val.parse::<i64>() {
            Ok(val) if val >= 0 && (val as u64) >= Duration::from_secs(24 * 60 * 60).as_secs() => {
                DateTime::from_timestamp(val, 0).map(RetryAfterHeaderValue::Timestamp)
            },
            Ok(val) if val >= 0 => {
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(val as u64)))
            },
            _ => None,
        })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use crate::http::StatusCode;
    use rstest::rstest;

    use super::*;

    mod retry_after_header_value {
        use super::*;

        mod from_response {
            use super::*;

            #[rstest]
            #[case::no_retry_headers(None, None, None, None)]
            #[case::valid_retry_after_header(
                Some("42"), None, None,
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::valid_x_ratelimit_reset_header(
                None, Some("23"), None,
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(23)))
            )]
            #[case::valid_x_rate_limit_reset_header(
                None, None, Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(66)))
            )]
            #[case::valid_retry_after_and_x_ratelimit_reset_headers(
                Some("42"), Some("23"), None,
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::valid_retry_after_and_x_rate_limit_reset_headers(
                Some("42"), None, Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::valid_x_ratelimit_reset_and_x_rate_limit_reset_headers(
                None, Some("23"), Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(23)))
            )]
            #[case::valid_headers_of_all_types(
                Some("42"), Some("23"), Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::invalid_retry_after_and_valid_x_ratelimit_reset_headers(
                Some("quarante-deux"), Some("23"), None,
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(23)))
            )]
            #[case::invalid_retry_after_and_valid_x_rate_limit_reset_headers(
                Some("quarante-deux"), None, Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(66)))
            )]
            #[case::invalid_x_ratelimit_reset_and_valid_x_rate_limit_reset_headers(
                None, Some("vingt-trois"), Some("66"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(66)))
            )]
            #[case::valid_retry_after_and_invalid_x_ratelimit_reset_headers(
                Some("42"), Some("vingt-trois"), None,
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::valid_retry_after_and_invalid_x_rate_limit_reset_headers(
                Some("42"), None, Some("soixante-six"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)))
            )]
            #[case::valid_x_ratelimit_reset_and_invalid_x_rate_limit_reset_headers(
                None, Some("23"), Some("soixante-six"),
                Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(23)))
            )]
            #[case::invalid_headers_of_all_type(
                Some("quarante-deux"), Some("vingt-trois"), Some("soixante-six"), None)]
            fn with(
                #[case] retry_after_header: Option<&str>,
                #[case] x_ratelimit_reset_header: Option<&str>,
                #[case] x_rate_limit_reset_header: Option<&str>,
                #[case] expected: Option<RetryAfterHeaderValue>,
            ) {
                let mut response = http::Response::builder()
                    .status(StatusCode::NO_CONTENT);
                if let Some(retry_after) = retry_after_header {
                    response = response.header(RETRY_AFTER, retry_after);
                }
                if let Some(x_ratelimit_reset) = x_ratelimit_reset_header {
                    response = response.header(X_RATELIMIT_RESET, x_ratelimit_reset);
                }
                if let Some(x_rate_limit_reset) = x_rate_limit_reset_header {
                    response = response.header(X_RATE_LIMIT_RESET, x_rate_limit_reset);
                }
                let response: Response = response.body("").unwrap().into();

                let actual = RetryAfterHeaderValue::from_response(&response);
                assert_eq!(actual, expected);
            }
        }

        mod into_sleep_time {
            use mock_instant::thread_local::MockClock;
            use super::*;

            fn timestamp_in_n_secs(secs: u64) -> DateTime<Utc> {
                std::time::SystemTime::from(SystemTime::now() + Duration::from_secs(secs)).into()
            }

            fn rfc2822_date_in_n_secs(secs: u64) -> String {
                timestamp_in_n_secs(secs).to_rfc2822()
            }

            #[rstest]
            #[case::timestamp_value_no_date_header(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(42)),
                None,
                None,
                Some(Duration::from_secs(42)),
            )]
            #[case::timestamp_value_no_date_header_and_non_zero_now(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(42)),
                None,
                Some(Duration::from_secs(11)),
                Some(Duration::from_secs(31)),
            )]
            #[case::timestamp_value_with_date_header(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(42)),
                Some(rfc2822_date_in_n_secs(23)),
                None,
                Some(Duration::from_secs(19)),
            )]
            #[case::timestamp_value_with_date_header_and_non_zero_now(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(42)),
                Some(rfc2822_date_in_n_secs(23)),
                Some(Duration::from_secs(11)),
                Some(Duration::from_secs(19)),
            )]
            #[case::timestamp_value_in_the_past_no_date_header(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(7)),
                None,
                Some(Duration::from_secs(11)),
                None,
            )]
            #[case::timestamp_value_in_the_past_with_date_header(
                RetryAfterHeaderValue::Timestamp(timestamp_in_n_secs(7)),
                Some(rfc2822_date_in_n_secs(23)),
                None,
                None,
            )]
            #[case::sleep_time_value_no_date_header(
                RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)),
                None,
                None,
                Some(Duration::from_secs(42)),
            )]
            #[case::sleep_time_value_no_date_header_and_non_zero_now(
                RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)),
                None,
                Some(Duration::from_secs(11)),
                Some(Duration::from_secs(42)),
            )]
            #[case::sleep_time_value_with_date_header(
                RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)),
                Some(rfc2822_date_in_n_secs(23)),
                None,
                Some(Duration::from_secs(42)),
            )]
            #[case::sleep_time_value_with_date_header_and_non_zero_now(
                RetryAfterHeaderValue::SleepTime(Duration::from_secs(42)),
                Some(rfc2822_date_in_n_secs(23)),
                Some(Duration::from_secs(11)),
                Some(Duration::from_secs(42)),
            )]
            fn with(
                #[case] header_value: RetryAfterHeaderValue,
                #[case] date_header: Option<String>,
                #[case] clock_now: Option<Duration>,
                #[case] expected: Option<Duration>,
            ) {
                if let Some(clock_now) = clock_now {
                    MockClock::set_system_time(clock_now);
                }

                let mut response = http::Response::builder()
                    .status(StatusCode::NO_CONTENT);
                if let Some(date_header) = date_header {
                    response = response.header(DATE, date_header);
                }
                let response: Response = response.body("").unwrap().into();

                let actual = header_value.into_sleep_time(&response);
                assert_eq!(actual, expected);
            }
        }
    }

    mod parse_retry_after_header {
        use super::*;

        #[rstest]
        #[case::non_negative_integer("42", Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(42))))]
        #[case::negative_integer("-42", None)]
        #[case::zero("0", Some(RetryAfterHeaderValue::SleepTime(Duration::ZERO)))]
        #[case::rfc2822_date(
            "Wed, 21 Oct 2015 07:28:00 GMT",
            {
                let expected = DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT").unwrap();
                Some(RetryAfterHeaderValue::Timestamp(expected.into()))
            },
        )]
        #[case::invalid_rfc2822_date("Foo, 21 Oct 2015 07:28:00 GMT", None)]
        #[case::invalid_string("The quick brown fox jumped over the lazy dog", None)]
        #[case::empty_string("", None)]
        fn with(#[case] val: &'static str, #[case] expected: Option<RetryAfterHeaderValue>) {
            let val = HeaderValue::from_static(val);
            let actual = parse_retry_after_header(&val);
            assert_eq!(actual, expected);
        }
    }

    mod parse_x_rate_limit_reset_header {
        use super::*;

        #[rstest]
        #[case::negative_integer("-1", None)]
        #[case::non_negative_number_of_seconds("23", Some(RetryAfterHeaderValue::SleepTime(Duration::from_secs(23))))]
        #[case::zero("0", Some(RetryAfterHeaderValue::SleepTime(Duration::ZERO)))]
        #[case::unix_timestamp(
            DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT").unwrap().timestamp().to_string(),
            {
                let expected = DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT").unwrap();
                Some(RetryAfterHeaderValue::Timestamp(expected.into()))
            },
        )]
        #[case::invalid_string("The quick brown fox jumped over the lazy dog", None)]
        #[case::empty_string("", None)]
        fn with<S>(#[case] val: S, #[case] expected: Option<RetryAfterHeaderValue>)
        where
            S: AsRef<str>,
        {
            let val = HeaderValue::from_str(val.as_ref()).unwrap();
            let actual = parse_x_rate_limit_reset_header(&val);
            assert_eq!(actual, expected);
        }
    }
}
