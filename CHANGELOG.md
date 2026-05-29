## Changes

### v0.9.0 (2026/05/29)
#### Breaking Change
- add fields to `LineOptions` and mark it `#[non_exhaustive]`. From outside the crate it can no longer be built with a struct literal (including `..Default::default()`); use `LineOptions::default()` together with the `with_*` builder methods instead
#### New Features
- add on_request / on_response callbacks to LineOptions for request/response logging
  - `LineRequestLog` / `LineResponseLog` expose their fields via accessor methods (`headers()`, `body()`, `status_code()`, `body_was_json()`)
  - `LineRequestLog::headers()` returns `Option<&HeaderMap>` so a header-capture failure is distinguishable from empty headers
  - `LineRequestLog::headers_redacted()` returns a header copy with the `Authorization` value masked
  - `LineRequestLog::body_redacted()` / `LineResponseLog::body_redacted()` mask known secret body keys (`client_secret`, `access_token`, `refresh_token`, `code`, `code_verifier`, `id_token`, `userAccessToken`; see `REDACTED_BODY_KEYS`)
  - `LineResponseLog::body_was_json()` distinguishes a parsed-JSON body from a raw-text body wrapped in `Value::String`
  - request-body serialization failures are surfaced as `{"_serialize_error": "..."}` rather than collapsing to `Value::Null` (which means "no body")
  - response body-read failures now propagate as `Error::Reqwest` instead of being swallowed into an empty body
  - add `with_prefix_url` / `with_timeout_duration` / `with_try_count` / `with_retry_duration` builders
  - note: callbacks are `#[serde(skip)]`, so serializing then deserializing a `LineOptions` drops them; set them via `with_on_request` / `with_on_response`
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