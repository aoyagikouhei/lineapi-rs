use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use chrono::prelude::*;

use super::{
    LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/messaging-api/#get-statistics-per-unit
const URL: &str = "/v2/bot/insight/message/event/aggregation";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    pub custom_aggregation_unit: String,
    pub from: String,
    pub to: String,
}

impl QueryParams {
    pub fn new(custom_aggregation_unit: &str) -> Self {
        let to = Local::now();
        let from = to - chrono::Duration::days(30);
        Self {
            custom_aggregation_unit: custom_aggregation_unit.to_owned(),
            from: from.format("%Y%m%d").to_string(),
            to: to.format("%Y%m%d").to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub unique_impression: Option<u64>,
    pub unique_click: Option<u64>,
    pub unique_media_played: Option<u64>,
    pub unique_media_played_100_percent: Option<u64>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub seq: u64,
    pub impression: Option<u64>,
    pub unique_impression: Option<u64>,
    pub media_played: Option<u64>,
    pub media_played_25_percent: Option<u64>,
    pub media_played_50_percent: Option<u64>,
    pub media_played_75_percent: Option<u64>,
    pub media_played_100_percent: Option<u64>,
    pub unique_media_played: Option<u64>,
    pub unique_media_played_25_percent: Option<u64>,
    pub unique_media_played_50_percent: Option<u64>,
    pub unique_media_played_75_percent: Option<u64>,
    pub unique_media_played_100_percent: Option<u64>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Click {
    pub seq: u64,
    pub url: Option<String>,
    pub click: Option<u64>,
    pub unique_click: Option<u64>,
    pub unique_click_of_request: Option<u64>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody {
    pub overview: Overview,
    pub messages: Vec<Message>,
    pub clicks: Vec<Click>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn build(
    query_params: &QueryParams,
    channel_access_token: &str,
    options: &LineOptions,
) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url).query(query_params);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    query_params: &QueryParams,
    channel_access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(
        || build(query_params, channel_access_token, options),
        options,
        is_standard_retry,
    )
    .await
}

#[cfg(test)]
mod tests {
    use crate::messaging_api::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_get_v2_bot_insight_message_event_aggregation -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_get_v2_bot_insight_message_event_aggregation() {
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let query_params = super::QueryParams::new("promotion_a");
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        let (response, header) = super::execute(&query_params, &channel_access_token, &options)
            .await
            .unwrap();
        println!("{}", serde_json::to_value(response).unwrap());
        println!("{:?}", header);
    }
}
