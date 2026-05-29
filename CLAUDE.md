# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`lineapi` is a Rust client library (crate published to crates.io / docs.rs) for the LINE Messaging API and LINE Login API. Edition 2024. The public API is organized one Rust module per LINE endpoint.

## Commands

```bash
cargo build                       # build the library
cargo build --all-features        # build with the `mock` feature enabled
cargo clippy --all-features       # lint (the CHANGELOG tracks clippy passes as release steps)
cargo fmt                         # format (commits like "cargo fmt" are part of the workflow)

# Mock tests run fully offline (mockito spins up a local server). These are the
# only tests safe to run without credentials, and they require --all-features:
cargo test --all-features

# Run a single mock test (note --test-threads=1, used throughout this repo):
cargo test --all-features test_make_mock_post_v2_bot_message_push_success -- --nocapture --test-threads=1
```

### Live integration tests (hit the real LINE API)

Most `#[tokio::test]` blocks inside `src/messaging_api/*` and `src/line_login/*` call the production LINE API and read credentials from environment variables. They are **not** runnable in CI/offline and will fail without valid tokens. Each test's doc comment gives the exact invocation. Examples:

```bash
USER_ID=xxx CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_post_v2_bot_message_push -- --nocapture --test-threads=1
CHANNEL_ACCESS_CODE=xxx cargo test test_get_v2_bot_message_aggregation_list -- --nocapture --test-threads=1
```

Common env vars: `CHANNEL_ACCESS_CODE` (channel access token), `USER_ID` (target user). `LINE_API_PREFIX_URL` overrides the API base URL (used to point at a mock server).

## Architecture

### Per-endpoint module convention

Every LINE endpoint is its own file under `src/messaging_api/` or `src/line_login/`, and follows the same shape (see `src/messaging_api/post_v2_bot_message_push.rs` as the canonical example):

- `const URL` — the endpoint path, with a doc-comment link to the official LINE reference.
- `RequestBody` / `QueryParams` — `serde` structs with `#[serde(rename_all = "camelCase")]`. A `new(...)` constructor does client-side validation, returning `Err(Box<Error>)` (e.g. push messages cap at 5). Response structs carry `#[serde(flatten)] pub extra: HashMap<String, Value>` so unknown fields from LINE are never dropped.
- `build(...) -> RequestBuilder` — constructs the `reqwest` request, applying auth and timeout via the shared helpers.
- `execute(...) -> Result<(ResponseBody, LineResponseHeader), Box<Error>>` — calls the shared `execute_api` with the endpoint's retry predicate. POST endpoints also take an `Option<String> retry_key`.

When adding an endpoint: create `src/<area>/<endpoint>.rs`, register it as `pub mod <endpoint>;` in `src/messaging_api.rs` or `src/line_login.rs`, and (if the `mock` feature should cover it) add a matching `src/mock/<area>/<endpoint>.rs` plus its `pub mod` line in `src/mock/<area>.rs`.

### Shared core (`src/lib.rs`)

All endpoints funnel through `lib.rs`, which is where cross-cutting behavior lives — change it here, not per-endpoint:

- `LineOptions` — per-call config: `prefix_url`, `timeout_duration`, `try_count`, `retry_duration`. Getters supply defaults (1 try, zero durations = disabled).
- `execute_api` — the retry loop implementing LINE's [request-retry flow](https://developers.line.biz/ja/docs/messaging-api/retrying-api-request/). Exponential backoff (`2^i`) plus random jitter via `calc_retry_duration`. The `X-Line-Retry-Key` header is sent only when a `retry_key` is provided **and** `try_count > 1`. `is_standard_retry` (retry on 5xx + 429) is the predicate passed by most endpoints.
- `execute_api_raw` — single request + response parsing. Treats `409 CONFLICT` as success when `allow_conflict` is set (a retried push is considered delivered).
- `make_url`, `apply_auth` (Bearer token), `apply_timeout`, `make_line_header` (extracts `X-Line-Request-Id` / `X-Line-Accepted-Request-Id` into `LineResponseHeader`).

### Errors (`src/error.rs`)

`Error` is a `thiserror` enum; APIs return `Box<Error>` (boxed since v0.7.0 to keep result sizes small). `Error::Line` wraps LINE's structured `ErrorResponse`; `OtherJson` / `OtherText` cover unparseable bodies. `.status_code()` and `.make_json()` are the accessors callers use.

### Streaming / pagination

Paginated endpoints (e.g. `get_v2_bot_message_aggregation_list`) expose `make_stream(...)` (an `async_stream::try_stream!` that walks the `next` cursor up to `max_page_count`) and a convenience `execute_stream(...)` that collects the stream into a `Vec`.

### Mock feature (`mock`, off by default)

Enables `mockito` + `derive_builder`. Mirrors the endpoint tree under `src/mock/`. Each mock module exposes a `MockParams` builder and `make_mock(server, builder) -> Mock` that registers a mockito route (matching auth header + request body) returning canned responses. Tests point `LineOptions.prefix_url` at `server.url()`. Downstream consumers can depend on `lineapi` with `features = ["mock"]` to test their own integrations offline.

### OAuth / Login helpers

`src/line_login.rs` holds the `oauth_url(...)` helper (builds the authorize URL, with optional PKCE: validates the 43–128 char `code_verifier`, derives the S256 `code_challenge`) and the `Scope` enum. `examples/oauth/` is a standalone axum app (separate crate, not a workspace member) demonstrating the full PKCE login flow; run it with `LINE_CLIENT_ID`/`LINE_CLIENT_SECRET`/`LINE_REDIRECT_URI` set (see README).
