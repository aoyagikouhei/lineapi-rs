use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{apply_auth, apply_timeout, execute_api, make_url, LineOptions, LineResponseHeader};

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

pub fn build(body: &RequestBody, channel_access_token: &str, options: &Option<LineOptions>) -> RequestBuilder {
    let url = make_url(URL, &options);
    let client = reqwest::Client::new();
    let mut request_builder = client.post(&url).json(&body);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, &options);
    request_builder
}

pub async fn execute(body: RequestBody, channel_access_token: &str, options: &Option<LineOptions>) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(|| build(&body, channel_access_token, options), options).await
}