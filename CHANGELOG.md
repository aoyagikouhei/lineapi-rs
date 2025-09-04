## Changes

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