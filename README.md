# lineapi-rs

LINE API library.

[Documentation](https://docs.rs/lineapi)

## Changes
[CHANGELOG.md](https://github.com/aoyagikouhei/lineapi-rs/blob/main/CHANGELOG.md)

## Support API
- [get_v2_bot_insight_message_event_aggregation](https://developers.line.biz/ja/reference/messaging-api/#get-statistics-per-unit)
- [get_v2_bot_message_aggregation_info](https://developers.line.biz/ja/reference/messaging-api/#get-the-number-of-unit-name-types-assigned-during-this-month)
- [get_v2_bot_message_aggregation_list](https://developers.line.biz/ja/reference/messaging-api/#get-a-list-of-unit-names-assigned-during-this-month)
- [get_v2_bot_info](https://developers.line.biz/ja/reference/messaging-api/#get-bot-info)
- [get_v2_bot_message_quote_consumption](https://developers.line.biz/ja/reference/messaging-api/#get-consumption)
- [get_v2_bot_message_quote](https://developers.line.biz/ja/reference/messaging-api/#get-quota)
- [get_v2_bot_profile](https://developers.line.biz/ja/reference/messaging-api/#get-profile)
- [post_v2_bot_message_push](https://developers.line.biz/ja/reference/messaging-api/#send-push-message)
- [post_v2_bot_message_validate_push](https://developers.line.biz/ja/reference/messaging-api/#validate-message-objects-of-push-message)

## Features
- [retry](https://developers.line.biz/ja/docs/messaging-api/retrying-api-request/#flow-of-api-request-retry)
- Timeout Duration
- Retry Duration
- Mock
- Stream