use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{apply_auth, apply_timeout, execute_api, is_standard_retry, make_url, LineOptions, LineResponseHeader};

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
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
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
    execute_api(|| build(user_id, channel_access_token, options), options, is_standard_retry).await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::messaging_api::LineOptions;

    // USER_ID=aaa CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_get_v2_bot_profile -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_get_v2_bot_profile() {
        let subscriber = FmtSubscriber::builder()
            // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
            // will be written to stdout.
            .with_max_level(Level::DEBUG)
            // completes the builder.
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let user_id = std::env::var("USER_ID").unwrap();
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        let (response, header) = super::execute(&user_id, &channel_access_token, &options)
            .await
            .unwrap();
        println!("{}", serde_json::to_value(response).unwrap());
        println!("{:?}", header);
    }
}
