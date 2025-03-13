use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, make_url};

// https://developers.line.biz/ja/reference/messaging-api/#send-push-message
const URL: &str = "/v2/bot/message/push";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestBody {
    pub to: String,
    pub messages: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SentMessage {
    id: String,
    #[serde(alias = "quoteToken")]
    quote_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    #[serde(alias = "sentMessages")]
    sent_messages: Vec<SentMessage>,
    message: Option<String>, // リトライの時に入る可能性がある
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
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(|| build(&body, channel_access_token, options), options).await
}

#[cfg(test)]
mod tests {
    use crate::messaging_api::LineOptions;

    // USER_ID=xxx CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_post_v2_bot_message_push -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_post_v2_bot_message_push() {
        let user_id = std::env::var("USER_ID").unwrap();
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let body = super::RequestBody {
            to: user_id,
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
