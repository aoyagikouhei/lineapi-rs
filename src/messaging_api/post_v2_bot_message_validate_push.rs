use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{apply_auth, apply_timeout, error::Error, is_standard_retry, make_url, messaging_api::execute_api, LineOptions, LineResponseHeader};

// https://developers.line.biz/ja/reference/messaging-api/#validate-message-objects-of-push-message
const URL: &str = "/v2/bot/message/validate/push";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestBody {
    pub messages: Vec<serde_json::Value>,
}

pub fn build(
    body: &RequestBody,
    channel_access_token: &str,
    options: &LineOptions,
) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.post(&url).json(&body);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    body: RequestBody,
    channel_access_token: &str,
    options: &LineOptions,
) -> Result<(serde_json::Value, LineResponseHeader), Error> {
    execute_api(
        || build(&body, channel_access_token, options),
        options,
        is_standard_retry,
    )
    .await
}

#[cfg(test)]
mod tests {
    use crate::messaging_api::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_post_v2_bot_message_validate_push -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_post_v2_bot_message_validate_push() {
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let body = super::RequestBody {
            messages: vec![serde_json::json!({
                "type": "text",
                "text": "Hello, world!"
            })],
        };
        let (response, header) = super::execute(body, &channel_access_token, &options)
            .await
            .unwrap();
        println!("{:?}", response);
        println!("{:?}", header);
    }
}
