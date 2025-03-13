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

#[cfg(test)]
mod tests {
    use crate::messaging_api::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_get_v2_bot_message_quote_consumption -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_get_v2_bot_message_quote_consumption() {
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let (response, header) = super::execute(&channel_access_token, &options)
            .await
            .unwrap();
        println!("{:?}", response);
        println!("{:?}", header);
    }
}
