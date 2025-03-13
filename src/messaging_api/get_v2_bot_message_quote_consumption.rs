use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, make_url};

// https://developers.line.biz/ja/reference/messaging-api/#get-consumption
const URL: &str = "/v2/bot/message/quota/consumption";

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    #[serde(alias = "totalUsage")]
    pub total_usage: i64,
}

pub fn build(channel_access_token: &str, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    channel_access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(|| build(channel_access_token, options), options).await
}
