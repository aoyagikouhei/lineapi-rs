## Changes

### v0.11.0 (2026/06/04)
#### New Features
- add `method()` / `path()` / `query()` / `query_redacted()` accessors to `LineRequestLog`
  - `method()` returns `Option<&reqwest::Method>`, `path()` returns the URL path as `Option<&str>` (e.g. `/v2/bot/message/push`); both are `None` only when the request capture failed (same contract as `headers()`)
  - `query()` returns the **raw** URL query string (`Option<&str>`), which may carry secrets (GET verify puts `access_token` in the query); `query_redacted()` returns a copy with known secret keys masked via the same `REDACTED_BODY_KEYS` / `with_redacted_body_keys` allowlist (so GET verify's `access_token` is masked). `Debug` for `LineRequestLog` uses the redacted query

### v0.10.0 (2026/06/04)
#### Breaking Change
- the `with_*` setters moved off `LineOptions` onto a new `LineOptionsBuilder`; construct options with `LineOptions::builder().with_*(...).build()` instead of `LineOptions::default().with_*(...)`. `LineOptions::default()` (no-op config) and the `get_*` accessors are unchanged
#### New Features
- add `LineOptionsBuilder` (obtained via `LineOptions::builder()` / `LineOptionsBuilder::new()`) carrying all `with_*` setters and a `build()` finalizer
- add `LineOptionsBuilder::with_redacted_body_keys` to configure the keys masked by `body_redacted()`
  - when unset, the default remains `REDACTED_BODY_KEYS` (`client_secret`, `access_token`, `refresh_token`, `code`, `code_verifier`, `id_token`, `userAccessToken`)
  - the supplied keys **replace** the default set (not merged); include `REDACTED_BODY_KEYS` to keep them
  - keys are normalized to lowercase and matched case-insensitively; an empty set disables masking
  - add `LineOptions::get_redacted_body_keys()` returning the effective keys (the default when unset)
- move `LineOptions` / `LineRequestLog` / `LineResponseLog` and the log-redaction code into a new `option` module; the public types remain re-exported at the crate root (`lineapi::LineOptions` etc. keep working)

### v0.9.0 (2026/06/02)
#### Breaking Change
- add fields to `LineOptions` and mark it `#[non_exhaustive]`. From outside the crate it can no longer be built with a struct literal (including `..Default::default()`); use `LineOptions::default()` together with the `with_*` builder methods instead
- `LineOptions` config fields (`prefix_url`, `timeout_duration`, `try_count`, `retry_duration`) are now `pub(crate)`; read them via `get_prefix_url` / `get_timeout_duration` / `get_try_count` / `get_retry_duration` and set them via the `with_*` builders
#### New Features
- add on_request / on_response callbacks to LineOptions for request/response logging
  - `LineRequestLog` exposes its fields via accessor methods (`headers()`, `body()`); `LineResponseLog` via (`headers()`, `as_value()`, `status_code()`, `body_was_json()`)
  - the response body is modeled by the `ResponseBody { Json, Raw }` enum and observed via `as_value()`, which returns `Cow<serde_json::Value>` (borrowed for JSON bodies, an owned `Value::String` for raw bodies); the enum makes a "was-JSON" flag unable to disagree with the stored body
  - `LineRequestLog::headers()` returns `Option<&HeaderMap>` so a header-capture failure is distinguishable from empty headers
  - `LineRequestLog::headers_redacted()` returns a header copy with the `Authorization` value masked
  - `LineRequestLog::body_redacted()` / `LineResponseLog::body_redacted()` mask known secret body keys (`client_secret`, `access_token`, `refresh_token`, `code`, `code_verifier`, `id_token`, `userAccessToken`; see `REDACTED_BODY_KEYS`). Note this is an allowlist of exact key names: unknown keys (e.g. a future LINE field, or one arriving via `#[serde(flatten)] extra`) are NOT masked and pass through
  - `Debug` for `LineRequestLog` / `LineResponseLog` is implemented to redact secrets (the `Authorization` header and the known body keys above), so `{:?}` / `tracing::*(?log)` will not leak raw tokens
  - `LineResponseLog::body_was_json()` distinguishes a parsed-JSON body from a raw-text body wrapped in `Value::String`
  - request-body serialization failures are surfaced as `{"_serialize_error": "..."}` rather than collapsing to `Value::Null` (which means "no body")
  - response body-read failures now propagate as `Error::Reqwest` instead of being swallowed into an empty body
  - add `with_prefix_url` / `with_timeout_duration` / `with_try_count` / `with_retry_duration` builders
  - add `get_prefix_url()` returning the effective base URL (honoring `with_prefix_url`, then `LINE_API_PREFIX_URL`, then the default)
  - callbacks receive UN-redacted logs; mask with the `*_redacted()` helpers (or the redacting `Debug`) before logging. Callbacks fire per retry attempt (up to `try_count` times); for streaming endpoints (`make_stream` / `execute_stream`) they additionally fire once per page (the two axes multiply)
  - note: callbacks are `#[serde(skip)]`, so serializing then deserializing a `LineOptions` drops them; set them via `with_on_request` / `with_on_response`
  - a panic inside an `on_request` / `on_response` callback is caught and logged via `tracing::error!`; the API call keeps running (logging stays a side-channel)
  - `with_try_count(0)` is treated as `1` (at least one attempt) instead of returning an opaque error
#### Modify
- update rand 0.10
- update sha2 0.11
- update strum 0.28
- update futures-util 0.3.32

### v0.8.0 (2026/01/22)
#### Breaking Change
- update reqwest 0.13
- rm feature rustls-tls
- modify post_v2_bot_message_push add retry_key

### v0.7.0 (2025/09/08)
#### Breaking Change
- return Box<Error>

### v0.6.4 (2025/09/04)
#### New Features
- wrong release 0.6.3

### v0.6.3 (2025/09/04)
#### New Features
- add mock builder

### v0.6.2 (2025/08/21)
#### New Features
- add Clone for request and response

### v0.6.1 (2025/07/30)
#### New Features
- add oauth_url
- add examples oauth web

### v0.6.0 (2025/07/30)
#### New Features
- add line_login

#### Breaking Change
- move LineOptions to lib
- change visibility common functions

### v0.5.2 (2025/05/13)
#### Modify
- rename unique_media_played_100_percent in get_v2_bot_insight_message_event_aggregation.
- clippy

### v0.5.1 (2025/05/13)
#### Modify
- add execute_stream in get_v2_bot_message_aggregation_list.

### v0.5.0 (2025/05/12)
#### Breaking Change
- Message fields in get_v2_bot_insight_message_event_aggregation.

### v0.4.2 (2025/05/10)
#### Modify post_v2_bot_message_push add notification_disabled

### v0.4.1 (2025/05/08)
#### Modify get_v2_bot_insight_message_event_aggregation response

### v0.4.0 (2025/05/08)
#### Modify post_v2_bot_message_push
- add custom_aggregation_units
#### Add APIs
- get_v2_bot_insight_message_event_aggregation
- get_v2_bot_message_aggregation_info
- get_v2_bot_message_aggregation_list

### v0.3.0 (2025/04/25)
#### Modify retry check

### v0.2.6 (2025/03/24)
#### Modify timeout is_zero

### v0.2.5 (2025/03/24)
#### Modify Exponential Backoff add Jitter 

### v0.2.4 (2025/03/19)
#### Add Enum to_string

### v0.2.3 (2025/03/19)
#### Add API
- get_v2_bot_info

### v0.2.2 (2025/03/14)
#### Add Clone in ErrorResponse

### v0.2.1 (2025/03/14)
#### Add get status code from Error

### v0.2.0 (2025/03/14)
#### Modify enum Error
- separete other
#### Modify Response
- add extra field

### v0.1.2 (2025/03/13)
#### Modify Error Json

### v0.1.1 (2025/03/13)
#### Modify Error

### v0.1.0 (2025/03/13)
#### First release.
- post_v2_bot_message_validate_push
- post_v2_bot_message_push
- get_v2_bot_message_quote;
- get_v2_bot_message_quote_consumption;
- get_v2_bot_profile;