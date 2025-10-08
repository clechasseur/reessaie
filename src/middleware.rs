//! Middleware implementations

use crate::RetryAfterPolicy;
use crate::http::Extensions;
use crate::policy::detail::DefaultRetryableStrategyInner;
use crate::reqwest::{Request, Response};
use crate::reqwest_middleware::{Middleware, Next};
use crate::reqwest_retry::policies::ExponentialBackoff;
use crate::reqwest_retry::{RetryPolicy, RetryTransientMiddleware, RetryableStrategy};

/// _TODO_
pub struct RetryAfterMiddleware<P = ExponentialBackoff, S = DefaultRetryableStrategyInner>(
    RetryTransientMiddleware<RetryAfterPolicy<P, S>, RetryAfterPolicy<P, S>>,
)
where
    P: RetryPolicy + Send + Sync + 'static,
    S: RetryableStrategy + Send + Sync + 'static;

impl<P, S> RetryAfterMiddleware<P, S>
where
    P: RetryPolicy + Clone + Send + Sync + 'static,
    S: RetryableStrategy + Clone + Send + Sync + 'static,
{
    /// _TODO_
    pub fn new_with_policy(policy: RetryAfterPolicy<P, S>) -> Self {
        Self(RetryTransientMiddleware::new_with_policy_and_strategy(policy.clone(), policy.clone()))
    }
}

#[async_trait::async_trait]
impl<P, S> Middleware for RetryAfterMiddleware<P, S>
where
    P: RetryPolicy + Send + Sync + 'static,
    S: RetryableStrategy + Send + Sync + 'static,
{
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> crate::reqwest_middleware::Result<Response> {
        self.0.handle(req, extensions, next).await
    }
}
