//! # `reessaie`
//!
//! _TODO_

#![deny(missing_docs)]
#![deny(rustdoc::missing_crate_level_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg_hide))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod middleware;
pub mod policy;

pub use http;
pub use policy::RetryAfterPolicy;
pub use reqwest;
pub use reqwest_middleware;
pub use reqwest_retry;
