use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, make_url};

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
    execute_api(|| build(&body, channel_access_token, options), options).await
}
