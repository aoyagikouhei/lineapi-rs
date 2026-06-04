# lineapi-rs

LINE API library supporting both LINE Messaging API and LINE Login API.

[Documentation](https://docs.rs/lineapi)

## Changes
[CHANGELOG.md](https://github.com/aoyagikouhei/lineapi-rs/blob/main/CHANGELOG.md)

## Supported APIs

### LINE Messaging API
- [get_v2_bot_info](https://developers.line.biz/ja/reference/messaging-api/#get-bot-info)
- [get_v2_bot_insight_message_event_aggregation](https://developers.line.biz/ja/reference/messaging-api/#get-statistics-per-unit)
- [get_v2_bot_message_aggregation_info](https://developers.line.biz/ja/reference/messaging-api/#get-the-number-of-unit-name-types-assigned-during-this-month)
- [get_v2_bot_message_aggregation_list](https://developers.line.biz/ja/reference/messaging-api/#get-a-list-of-unit-names-assigned-during-this-month)
- [get_v2_bot_message_quote](https://developers.line.biz/ja/reference/messaging-api/#get-quota)
- [get_v2_bot_message_quote_consumption](https://developers.line.biz/ja/reference/messaging-api/#get-consumption)
- [get_v2_bot_profile](https://developers.line.biz/ja/reference/messaging-api/#get-profile)
- [post_v2_bot_message_push](https://developers.line.biz/ja/reference/messaging-api/#send-push-message)
- [post_v2_bot_message_validate_push](https://developers.line.biz/ja/reference/messaging-api/#validate-message-objects-of-push-message)

### LINE Login API (v0.6.0+)
- [get_friendship_v1_status](https://developers.line.biz/ja/reference/line-login/#get-friendship-status)
- [get_oauth2_v2_1_userinfo](https://developers.line.biz/ja/reference/line-login/#userinfo)
- [get_oauth2_v2_1_verify](https://developers.line.biz/ja/reference/line-login/#verify-access-token)
- [get_v2_profile](https://developers.line.biz/ja/reference/line-login/#get-user-profile)
- [post_oauth2_v2_1_revoke](https://developers.line.biz/ja/reference/line-login/#revoke-access-token)
- [post_oauth2_v2_1_token](https://developers.line.biz/ja/reference/line-login/#issue-access-token)
- [post_oauth2_v2_1_verify](https://developers.line.biz/ja/reference/line-login/#verify-id-token)
- [post_user_v1_deauthorize](https://developers.line.biz/ja/reference/line-login/#revoke-channelaccess-token-v2-1)
- oauth_url helper function (v0.6.1) - Generate OAuth authorization URL with PKCE support

## Features
- [Retry mechanism](https://developers.line.biz/ja/docs/messaging-api/retrying-api-request/#flow-of-api-request-retry) with exponential backoff
- Configurable timeout duration
- Configurable retry duration
- Mock support for testing
- Stream support for large data
- PKCE (Proof Key for Code Exchange) support for OAuth
- Request/response logging via `on_request` / `on_response` callbacks (v0.9.0), with built-in secret redaction (`headers_redacted` / `body_redacted` / `query_redacted`, and a redacting `Debug`); the request log also carries the `method()` / `path()` / `query()` of the call (v0.11.0)

## Logging

Build a `LineOptions` with `LineOptions::builder()` and attach `on_request` / `on_response` callbacks to observe every HTTP request and response. The callbacks receive **un-redacted** logs (the `Authorization` header and OAuth body secrets such as `client_secret` / `access_token` are present in the clear), so always mask before logging — use the `*_redacted()` helpers, or the `Debug` impl which redacts automatically.

```rust
use lineapi::LineOptions;

let options = LineOptions::builder()
    .with_on_request(|log| {
        // `log.method()` / `log.path()` identify the endpoint; `log.query()` is the raw
        // query string (may carry secrets — use `query_redacted()`).
        // `log.body()` / `log.headers()` / `log.query()` are UN-redacted — mask before logging.
        println!(
            "[LINE request] {:?} {:?} query={:?} body={}",
            log.method(),
            log.path(),
            log.query_redacted(),
            log.body_redacted(),
        );
    })
    .with_on_response(|_req, res| {
        println!("[LINE response] status={} body={}", res.status_code(), res.body_redacted());
    })
    .build();
```

All configuration (`with_prefix_url` / `with_timeout_duration` / `with_try_count` / `with_retry_duration` / `with_redacted_body_keys` / `with_on_request` / `with_on_response`) lives on `LineOptionsBuilder`; finish with `.build()`. For a no-op config, `LineOptions::default()` still works.

Notes:
- Callbacks fire once **per retry attempt** (up to `try_count`); for streaming endpoints (`make_stream` / `execute_stream`) they additionally fire once **per page**.
- `LineRequestLog` also exposes `method()` / `path()` / `query()`; `method()` and `path()` are `None` only when the request capture fails (same contract as `headers()`). `query()` is the raw URL query string and may contain secrets (GET verify puts `access_token` there) — use `query_redacted()`, which masks the same allowlist of keys as `body_redacted()`.
- `body_redacted()` masks an allowlist of known secret keys only (default: `REDACTED_BODY_KEYS`); unknown keys are not masked.
- Customize the masked keys with `LineOptionsBuilder::with_redacted_body_keys([...])`. It **replaces** the default set (it does not merge — include `REDACTED_BODY_KEYS` if you want to keep them). Keys are normalized to lowercase and matched case-insensitively; passing an empty set disables masking.
- A panic inside a callback is caught and logged via `tracing::error!`; the API call keeps running.

## Examples

### OAuth Web Application (`examples/oauth/`)
A complete web application demonstrating LINE Login integration with PKCE (Proof Key for Code Exchange).

**Features:**
- OAuth authorization URL generation with PKCE
- State parameter validation for security
- Authorization code exchange for access token
- User profile retrieval using access token
- Secure cookie-based session management
- Redacted request/response logging via `on_request` / `on_response` callbacks

**Usage:**
```bash
cd examples/oauth
LINE_CLIENT_ID=your_client_id \
LINE_CLIENT_SECRET=your_client_secret \
LINE_REDIRECT_URI=http://localhost:5173/oauth-line \
cargo run
```

**Flow:**
1. Visit `http://localhost:5173/` to start OAuth flow
2. Click the OAuth link to authorize with LINE
3. Get redirected back with user profile information

**Dependencies:**
- `axum` - Web framework
- `tower-cookies` - Cookie management
- `uuid` - Generate secure PKCE verifier and state