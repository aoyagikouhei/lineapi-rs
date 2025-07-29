use std::time::Duration;

use rand::{rngs::StdRng, Rng};
use reqwest::{header::{self, AUTHORIZATION}, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorResponse};

pub mod error;
pub mod messaging_api;
pub mod line_login;

const PREFIX_URL: &str = "https://api.line.me";
const ENV_KEY: &str = "LINE_API_PREFIX_URL";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LineResponseHeader {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_request_id: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LineOptions {
    pub prefix_url: Option<String>,
    pub timeout_duration: Option<Duration>,
    pub try_count: Option<u8>,
    pub retry_duration: Option<Duration>,
}

impl LineOptions {
    pub fn get_try_count(&self) -> u8 {
        self.try_count.unwrap_or(1)
    }

    pub fn get_retry_duration(&self) -> Duration {
        self.retry_duration.unwrap_or(Duration::from_secs(0))
    }

    pub fn get_timeout_duration(&self) -> Duration {
        self.timeout_duration.unwrap_or(Duration::from_secs(0))
    }
}

pub fn make_url(postfix_url: &str, options: &LineOptions) -> String {
    let default_prefix_url = std::env::var(ENV_KEY).unwrap_or_else(|_| PREFIX_URL.to_string());
    let prefix_url = if let Some(prefix_url) = &options.prefix_url {
        prefix_url
    } else {
        &default_prefix_url
    };
    format!("{}{}", prefix_url, postfix_url)
}

pub fn apply_auth(builder: RequestBuilder, channel_access_token: &str) -> RequestBuilder {
    builder.header(AUTHORIZATION, format!("Bearer {}", channel_access_token))
}

pub fn apply_timeout(builder: RequestBuilder, options: &LineOptions) -> RequestBuilder {
    let timeout_duration = options.get_timeout_duration();
    if timeout_duration.is_zero() {
        builder
    } else {
        builder.timeout(timeout_duration)
    }
}

pub fn is_standard_retry(status_code: StatusCode) -> bool {
    status_code.is_server_error() || status_code == StatusCode::TOO_MANY_REQUESTS
}

pub(crate) fn make_line_header(response: &Response) -> LineResponseHeader {
    let headers: &header::HeaderMap = response.headers();
    let request_id = headers
        .get("X-Line-Request-Id")
        .map(|it| it.to_str().unwrap_or(""))
        .unwrap_or("");
    let accepted_request_id = headers
        .get("X-Line-Accepted-Request-Id")
        .map(|it| it.to_str().unwrap_or("").to_string());
    LineResponseHeader {
        request_id: request_id.to_owned(),
        accepted_request_id,
    }
}

pub(crate) fn calc_retry_duration(retry_duration: Duration, try_count: u32, rng: &mut StdRng) -> Duration {
    // Jistter
    let jitter = Duration::from_millis(rng.random_range(0..100));

    // exponential backoff
    // 0の時1回、1の時2回、2の時4回、3の時8回
    let retry_count = 2u64.pow(try_count) as u32;
    retry_duration * retry_count + jitter
}

// APIを実行して一時的にエラーをハンドリングする
pub(crate) async fn execute_api_raw(
    builder: RequestBuilder,
    allow_conflict: bool,
) -> Result<(serde_json::Value, LineResponseHeader, StatusCode), Error> {
    let response = builder.send().await?;
    let status_code = response.status();
    let line_header = make_line_header(&response);
    let text = response.text().await.unwrap_or_default();
    let Ok(json) = serde_json::from_str(&text) else {
        return Err(Error::OtherText(text, status_code, line_header));
    };
    // コンフリクトしてもメッセージ送信はフォーマットが崩れないので成功とする
    if status_code.is_success() || (allow_conflict && status_code == StatusCode::CONFLICT) {
        Ok((json, line_header, status_code))
    } else {
        match serde_json::from_value::<ErrorResponse>(json.clone()) {
            Ok(error_response) => Err(Error::Line(error_response, status_code, line_header)),
            Err(_) => Err(Error::OtherJson(json, status_code, line_header)),
        }
    }
}
