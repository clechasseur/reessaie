# `reessaie`

[![CI](https://github.com/clechasseur/reessaie/actions/workflows/ci.yml/badge.svg?branch=main&event=push)](https://github.com/clechasseur/reessaie/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/clechasseur/reessaie/graph/badge.svg?token=LJZHJQnKqU)](https://codecov.io/gh/clechasseur/reessaie) [![Security audit](https://github.com/clechasseur/reessaie/actions/workflows/audit-check.yml/badge.svg?branch=main)](https://github.com/clechasseur/reessaie/actions/workflows/audit-check.yml) [![crates.io](https://img.shields.io/crates/v/reessaie.svg)](https://crates.io/crates/reessaie) [![downloads](https://img.shields.io/crates/d/reessaie.svg)](https://crates.io/crates/reessaie) [![docs.rs](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/reessaie) [![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

Companion library to [`reqwest-retry`](https://crates.io/crates/reqwest-retry) with helpers to use the [`Retry-After`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Retry-After) HTTP header to control the time between retries, if it's available.

Partially inspired by [`reqwest-retry-after`](https://crates.io/crates/reqwest-retry-after).

## Installing

Add `reessaie` to your dependencies:

```toml
[dependencies]
reessaie = "3.0.0"
```

or by running:

```shell
cargo add reesaie
```

## Example

```rust
use anyhow::Context;
use reessaie::{RetryAfterMiddleware, RetryAfterPolicy};
use reqwest::Client;
use reqwest_middleware::ClientBuilder;

async fn get_with_retries(url: &str, max_retries: u32) -> Result<String, anyhow::Error> {
    let policy = RetryAfterPolicy::with_max_retries(max_retries);
    let client = ClientBuilder::new(Client::new())
        .with(RetryAfterMiddleware::new_with_policy(policy))
        .build();

    Ok(client
        .get(url)
        .send()
        .await
        .with_context(|| format!("error getting {url}"))?
        .text()
        .await
        .with_context(|| format!("error getting text for {url}"))?)
}
```

For more information, see [the docs](https://docs.rs/reessaie).

## Minimum Rust version

`reessaie` currently builds on Rust 1.88 or newer.
