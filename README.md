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

## Examples

### OAuth Web Application (`examples/oauth/`)
A complete web application demonstrating LINE Login integration with PKCE (Proof Key for Code Exchange).

**Features:**
- OAuth authorization URL generation with PKCE
- State parameter validation for security
- Authorization code exchange for access token
- User profile retrieval using access token
- Secure cookie-based session management

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