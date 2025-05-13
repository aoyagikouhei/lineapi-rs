use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{
    LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/messaging-api/#send-push-message
const URL: &str = "/v2/bot/message/push";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    pub to: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_aggregation_units: Option<Vec<String>>,
}

impl RequestBody {
    pub fn new(to: &str, messages: Vec<serde_json::Value>) -> Result<Self, Box<Error>> {
        if to.is_empty() {
            return Err(Box::new(Error::Invalid("to is empty".to_string())));
        }
        if messages.is_empty() {
            return Err(Box::new(Error::Invalid("messages is empty".to_string())));
        }
        if messages.len() > 5 {
            return Err(Box::new(Error::Invalid(format!(
                "messages is too long: {}",
                messages.len()
            ))));
        }
        Ok(Self {
            to: to.to_string(),
            messages,
            notification_disabled: None,
            custom_aggregation_units: None,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SentMessage {
    id: String,
    #[serde(alias = "quoteToken")]
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_token: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    #[serde(alias = "sentMessages")]
    sent_messages: Vec<SentMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>, // リトライの時に入る可能性がある
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
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

    // USER_ID=xxx CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_post_v2_bot_message_push -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_post_v2_bot_message_push() {
        let user_id = std::env::var("USER_ID").unwrap();
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let mut body = super::RequestBody::new(
            &user_id,
            vec![serde_json::json!({
                "type": "text",
                "text": "Hello, world! http://www.yahoo.co.jp"
            })],
        )
        .unwrap();
        body.notification_disabled = Some(true);
        body.custom_aggregation_units = Some(vec!["promotion_a".to_owned()]);
        let (response, header) = super::execute(body, &channel_access_token, &options)
            .await
            .unwrap();
        println!("{:?}", response);
        println!("{:?}", header);
    }
}
