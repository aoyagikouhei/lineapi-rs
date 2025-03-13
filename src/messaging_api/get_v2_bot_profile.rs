use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, make_url};

// https://developers.line.biz/ja/reference/messaging-api/#get-profile
const URL: &str = "/v2/bot/profile";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody {
    pub display_name: String,
    pub user_id: String,
    pub language: Option<String>,
    pub picture_url: Option<String>,
    pub status_message: Option<String>,
}

pub fn build(user_id: &str, channel_access_token: &str, options: &LineOptions) -> RequestBuilder {
    let url = make_url(&format!("{}/{}", URL, user_id), options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    user_id: &str,
    channel_access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(|| build(user_id, channel_access_token, options), options).await
}

#[cfg(test)]
mod tests {
    use crate::messaging_api::LineOptions;

    // USER_ID=aaa CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_get_v2_bot_profile -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_get_v2_bot_profile() {
        let user_id = std::env::var("USER_ID").unwrap();
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions::default();
        let (response, header) = super::execute(&user_id, &channel_access_token, &options)
            .await
            .unwrap();
        println!("{:?}", response);
        println!("{:?}", header);
    }
}
