//! Middleware implementations

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::{
    DefaultRetryableStrategy, RetryPolicy, RetryTransientMiddleware, RetryableStrategy,
};

use crate::RetryAfterPolicy;

/// Middleware wrapping a [`RetryTransientMiddleware`] to allow easier use of [`RetryAfterPolicy`].
///
/// See documentation for [`RetryAfterPolicy`] for information on how the retry policy works.
///
/// # Usage
///
/// Create a [`RetryAfterPolicy`], then pass it to [`new_with_policy`]:
///
/// ```
/// # use reqwest::Client;
/// # use reqwest_middleware::ClientBuilder;
/// # use reessaie::{RetryAfterMiddleware, RetryAfterPolicy};
///
/// let policy = RetryAfterPolicy::with_max_retries(5);
/// let client = ClientBuilder::new(Client::new())
///     .with(RetryAfterMiddleware::new_with_policy(policy))
///     .build();
/// ```
///
/// [`new_with_policy`]: RetryAfterMiddleware::new_with_policy
pub struct RetryAfterMiddleware<P = ExponentialBackoff, S = DefaultRetryableStrategy>(
    RetryTransientMiddleware<RetryAfterPolicy<P, S>, RetryAfterPolicy<P, S>>,
)
where
    RetryAfterPolicy<P, S>: RetryPolicy + RetryableStrategy + Send + Sync + 'static;

impl<P, S> RetryAfterMiddleware<P, S>
where
    RetryAfterPolicy<P, S>: RetryPolicy + RetryableStrategy + Clone + Send + Sync + 'static,
{
    /// Creates a [`RetryAfterMiddleware`] wrapping the given [`RetryAfterPolicy`].
    ///
    /// See [struct documentation](RetryAfterMiddleware) for usage details.
    pub fn new_with_policy(policy: RetryAfterPolicy<P, S>) -> Self {
        Self(RetryTransientMiddleware::new_with_policy_and_strategy(policy.clone(), policy.clone()))
    }
}

#[async_trait]
impl<P, S> Middleware for RetryAfterMiddleware<P, S>
where
    RetryAfterPolicy<P, S>: RetryPolicy + RetryableStrategy + Send + Sync + 'static,
{
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        self.0.handle(req, extensions, next).await
    }
}
