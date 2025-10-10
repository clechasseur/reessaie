//! # `reessaie`
//!
//! Companion library to [`reqwest_retry`] with helpers to use the [`Retry-After`] HTTP header to
//! control the time between retries, if it's available.
//!
//! Partially inspired by [`reqwest_retry_after`].
//!
//! For more information on goals and usage, see [`RetryAfterPolicy`] and [`RetryAfterMiddleware`].
//!
//! # Example
//!
//! ```
//! use anyhow::Context;
//! use reessaie::{RetryAfterMiddleware, RetryAfterPolicy};
//! use reqwest::Client;
//! use reqwest_middleware::ClientBuilder;
//!
//! async fn get_with_retries(url: &str, max_retries: u32) -> Result<String, anyhow::Error> {
//!     let policy = RetryAfterPolicy::with_max_retries(max_retries);
//!     let client = ClientBuilder::new(Client::new())
//!         .with(RetryAfterMiddleware::new_with_policy(policy))
//!         .build();
//!
//!     Ok(client
//!         .get(url)
//!         .send()
//!         .await
//!         .with_context(|| format!("error getting {url}"))?
//!         .text()
//!         .await
//!         .with_context(|| format!("error getting text for {url}"))?)
//! }
//! ```
//!
//! [`reqwest_retry`]: https://crates.io/crates/reqwest-retry
//! [`Retry-After`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After
//! [`reqwest_retry_after`]: https://crates.io/crates/reqwest-retry-after

#![deny(missing_docs)]
#![deny(rustdoc::missing_crate_level_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg_hide))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod middleware;
mod policy;

pub use http;
pub use middleware::RetryAfterMiddleware;
pub use policy::RetryAfterPolicy;
pub use reqwest;
pub use reqwest_middleware;
pub use reqwest_retry;
