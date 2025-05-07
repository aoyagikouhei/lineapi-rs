use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use super::{
    LineOptions, LineResponseHeader, apply_auth, apply_timeout, execute_api, is_standard_retry,
    make_url,
};
use crate::error::Error;

use async_stream::try_stream;
use futures_util::Stream;

// https://developers.line.biz/ja/reference/messaging-api/#get-a-list-of-unit-names-assigned-during-this-month
const URL: &str = "/v2/bot/message/aggregation/list";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

impl QueryParams {
    pub fn new(start: &str) -> Self {
        Self {
            limit: Some(100),
            start: if start.is_empty() {
                None
            } else {
                Some(start.to_string())
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody {
    pub custom_aggregation_units: Vec<String>,
    pub next: Option<String>,
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

pub fn make_stream(
    query_params: &QueryParams,
    channel_access_token: &str,
    options: &LineOptions,
    max_page_count: u64,
) -> impl Stream<Item = Result<String, Error>> {
    try_stream! {
        let mut query_params = query_params.clone();
        let mut current_page_count = 0;
        loop {
            // 通常のAPI呼び出し
            let (result, _) = execute(&query_params, channel_access_token, options).await?;

            // 空なら終了
            if result.custom_aggregation_units.is_empty() {
                break;
            }

            // データがあったら1件づつ返す
            for item in result.custom_aggregation_units {
                yield item;
            }

            // 次のページがない場合は終了
            if result.next.is_none() {
                break;
            }

            // 次のページに進む
            query_params.start = result.next;

            // 最大ページ数を超えたら終了
            current_page_count += 1;
            if current_page_count > max_page_count {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_util::{pin_mut, stream::TryStreamExt};

    use crate::messaging_api::LineOptions;

    // CHANNEL_ACCESS_CODE=xxx cargo test test_get_v2_bot_message_aggregation_list -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_get_v2_bot_message_aggregation_list() {
        let channel_access_token = std::env::var("CHANNEL_ACCESS_CODE").unwrap();
        let query_params = super::QueryParams::new("");
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        let stream = super::make_stream(&query_params, &channel_access_token, &options, 100);
        pin_mut!(stream); // おまじない

        loop {
            match stream.try_next().await {
                Ok(item) => match item {
                    Some(item) => {
                        println!("item: {}", item);
                    }
                    None => {
                        println!("no more items");
                        break;
                    }
                },
                Err(e) => {
                    println!("error: {}", e);
                    break;
                }
            }
        }
    }
}
