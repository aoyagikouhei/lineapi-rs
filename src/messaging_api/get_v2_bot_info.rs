use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::error::Error;

use super::{apply_auth, apply_timeout, execute_api, is_standard_retry, make_url, LineOptions, LineResponseHeader};

// https://developers.line.biz/ja/reference/messaging-api/#get-bot-info
const URL: &str = "/v2/bot/info";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Display)]
pub enum ChatMode {
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "bot")]
    Bot,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Display)]
pub enum MarkAsReadMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "manual")]
    Manual,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody {
    pub user_id: String,
    pub basic_id: String,
    pub premium_id: Option<String>,
    pub display_name: String,
    pub picture_url: Option<String>,
    pub chat_mode: ChatMode,
    pub mark_as_read_mode: MarkAsReadMode,
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
    execute_api(|| build(channel_access_token, options), options, is_standard_retry).await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::messaging_api::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_messaging_api_get_v2_bot_info -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_messaging_api_get_v2_bot_info() {
        let subscriber = FmtSubscriber::builder()
            // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
            // will be written to stdout.
            .with_max_level(Level::DEBUG)
            // completes the builder.
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        let (response, header) = super::execute(&channel_access_token, &options)
            .await
            .unwrap();
        println!("{}", serde_json::to_value(response).unwrap());
        println!("{:?}", header);
    }
}
