use std::time::Duration;

use reqwest::{header::AUTHORIZATION, RequestBuilder};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::error::{Error, ErrorResponse};

pub mod get_v2_bot_message_quote_consumption;
pub mod get_v2_bot_message_quote;
pub mod get_v2_bot_profile;
pub mod post_v2_bot_message_push;
pub mod post_v2_bot_message_validate_push;

const PREFIX_URL: &str = "https://api.line.me";
const ENV_KEY: &str = "LINE_API_PREFIX_URL";
const HEADER_RETRY_KEY: &str = "X-Line-Retry-Key";

#[derive(Debug)]
pub struct LineResponseHeader {
    pub request_id: String,
    pub accepted_request_id: Option<String>,
}


#[derive(Debug)]
pub struct LineOptions {
    pub prefix_url: Option<String>,
    pub timeout_duration: Option<Duration>,
    pub try_count: Option<u8>,
    pub retry_duration: Option<Duration>,
}

pub fn make_url(postfix_url: &str, options: &Option<LineOptions>) -> String {
    let default_prefix_url = std::env::var(ENV_KEY).unwrap_or_else(|_| PREFIX_URL.to_string());
    let prefix_url = match options {
        Some(options) => {
            if let Some(prefix_url) = &options.prefix_url {
                prefix_url
            } else {
                &default_prefix_url
            }
        }
        None => &default_prefix_url,
    };
    format!("{}{}", prefix_url, postfix_url)
}

pub fn apply_auth(builder: RequestBuilder, channel_access_token: &str) -> RequestBuilder {
    builder.header(
        AUTHORIZATION,
        format!("Bearer {}", channel_access_token),
    )
}

pub fn apply_timeout(builder: RequestBuilder, options: &Option<LineOptions>) -> RequestBuilder {
    let Some(options) = options else {
        return builder;
    };
    let Some(timeout_duration) = options.timeout_duration else {
        return builder;
    };
    builder.timeout(timeout_duration)
}

pub async fn execute_api<T>(f: impl Fn() -> RequestBuilder, options: &Option<LineOptions>) -> Result<(T, LineResponseHeader), crate::error::Error> 
where
    T: DeserializeOwned,
{
    // リトライ処理
    // https://developers.line.biz/ja/docs/messaging-api/retrying-api-request/#flow-of-api-request-retry
    let mut res = Err(Error::Timeout);
    let retry_key = Uuid::now_v7().to_string();
    let try_count = options.as_ref().map(|it| it.try_count).unwrap_or(Some(0)).unwrap_or(0);
    let retry_duration = options.as_ref().map(|it| it.retry_duration).unwrap_or(Some(Duration::from_secs(0))).unwrap_or(Duration::from_secs(0));
    for i in 0..try_count {
        // リクエスト準備
        let mut builder = f();
        if try_count > 0 {
            // リトライ回数がある場合はリトライキーをヘッダーに追加
            builder = builder.header(HEADER_RETRY_KEY, &retry_key);
        }
        match execute_api_raw(builder).await {
            Ok((json, header)) => {
                let data = serde_json::from_value(json)?;
                res = Ok((data, header));
                break;
            }
            Err(err) => {
                if i >= try_count - 1 {
                    // リトライ回数がオーバーしたので失敗にする
                    res = Err(err.into());
                } else {
                    if retry_duration.as_secs() > 0 {
                        // リトライ間隔がある場合は待つ
                        tokio::time::sleep(retry_duration).await;
                    }
                }
            }
        }
    }
    res
}

async fn execute_api_raw(builder: RequestBuilder) -> Result<(serde_json::Value, LineResponseHeader), Error> {
    let response = builder.send().await?;
    let status_code = response.status();
    let headers = response.headers().clone();
    let text = response.text().await?;
    let Ok(json) = serde_json::from_str(&text) else {
        return Err(Error::Other(text, status_code));
    };
    if status_code.is_success() {
        let request_id = headers.get("X-Line-Request-Id").map(|it| it.to_str().unwrap_or("")).unwrap_or("");
        let accepted_request_id = match headers.get("X-Line-Accepted-Request-Id") {
            Some(it) => Some(it.to_str().unwrap_or("").to_string()),
            None => None,
        };
        Ok((json, LineResponseHeader { request_id: request_id.to_owned(), accepted_request_id }))
    } else {
        let Ok(error_response) = serde_json::from_value::<ErrorResponse>(json) else {
            return Err(Error::Other(text, status_code));
        };
        Err(Error::Line(error_response, status_code))
    }
}