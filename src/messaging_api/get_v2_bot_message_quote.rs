use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    LineOptions, LineResponseHeader, apply_auth, apply_timeout, error::Error, execute_api,
    is_standard_retry, make_url,
};

// https://developers.line.biz/ja/reference/messaging-api/#get-quota
const URL: &str = "/v2/bot/message/quota";

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    #[serde(alias = "type")]
    pub type_code: String,
    pub value: Option<i64>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
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
    execute_api(
        || build(channel_access_token, options),
        options,
        is_standard_retry,
        false,
    )
    .await
}

#[cfg(test)]
mod tests {
    use crate::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_get_v2_bot_message_quote -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_get_v2_bot_message_quote() {
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let (response, header) = super::execute(&channel_access_token, &options)
            .await
            .unwrap();
        println!("{response:?}");
        println!("{header:?}");
    }
}
