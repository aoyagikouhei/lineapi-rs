use std::time::Duration;

use rand::{Rng, SeedableRng, rngs::StdRng};
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{self, AUTHORIZATION},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::{Error, ErrorResponse};

pub mod error;
pub mod line_login;
pub mod messaging_api;

#[cfg(feature = "mock")]
pub mod mock;

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

pub(crate) fn make_url(postfix_url: &str, options: &LineOptions) -> String {
    let default_prefix_url = std::env::var(ENV_KEY).unwrap_or_else(|_| PREFIX_URL.to_string());
    let prefix_url = if let Some(prefix_url) = &options.prefix_url {
        prefix_url
    } else {
        &default_prefix_url
    };
    format!("{prefix_url}{postfix_url}")
}

pub(crate) fn apply_auth(builder: RequestBuilder, channel_access_token: &str) -> RequestBuilder {
    builder.header(AUTHORIZATION, format!("Bearer {channel_access_token}"))
}

pub(crate) fn apply_timeout(builder: RequestBuilder, options: &LineOptions) -> RequestBuilder {
    let timeout_duration = options.get_timeout_duration();
    if timeout_duration.is_zero() {
        builder
    } else {
        builder.timeout(timeout_duration)
    }
}

pub(crate) fn is_standard_retry(status_code: StatusCode) -> bool {
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

pub(crate) fn calc_retry_duration(
    retry_duration: Duration,
    try_count: u32,
    rng: &mut StdRng,
) -> Duration {
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
) -> Result<(serde_json::Value, LineResponseHeader, StatusCode), Box<Error>> {
    let response = builder
        .send()
        .await
        .map_err(|err| Box::new(Error::Reqwest(err)))?;
    let status_code = response.status();
    let line_header = make_line_header(&response);
    let text = response.text().await.unwrap_or_default();
    let Ok(json) = serde_json::from_str(&text) else {
        return Err(Box::new(Error::OtherText(text, status_code, line_header)));
    };
    // コンフリクトしてもメッセージ送信はフォーマットが崩れないので成功とする
    if status_code.is_success() || (allow_conflict && status_code == StatusCode::CONFLICT) {
        Ok((json, line_header, status_code))
    } else {
        match serde_json::from_value::<ErrorResponse>(json.clone()) {
            Ok(error_response) => Err(Box::new(Error::Line(
                error_response,
                status_code,
                line_header,
            ))),
            Err(_) => Err(Box::new(Error::OtherJson(json, status_code, line_header))),
        }
    }
}

const HEADER_RETRY_KEY: &str = "X-Line-Retry-Key";

pub(crate) async fn execute_api<T, F>(
    f: impl Fn() -> RequestBuilder,
    options: &LineOptions,
    is_retry: F,
    retry_key: Option<String>,
) -> Result<(T, LineResponseHeader), Box<Error>>
where
    T: DeserializeOwned,
    F: Fn(StatusCode) -> bool,
{
    // リトライ処理
    // https://developers.line.biz/ja/docs/messaging-api/retrying-api-request/#flow-of-api-request-retry
    let mut res = Err(Error::Invalid("fail loop".to_string()));
    let try_count = options.get_try_count();
    let retry_duration: Duration = options.get_retry_duration();
    let mut rng = StdRng::from_os_rng();
    for i in 0..try_count {
        // リクエスト準備
        let mut builder = f();
        // リトライ処理はtry_countが1以上の場合のみ
        if let Some(retry_key) = &retry_key {
            if try_count > 1 {
                // リトライ回数がある場合はリトライキーをヘッダーに追加
                builder = builder.header(HEADER_RETRY_KEY, retry_key);
            }
        }
        match execute_api_raw(builder, retry_key.is_some()).await {
            Ok((json, line_header, status_code)) => {
                res = match serde_json::from_value(json.clone()) {
                    // フォーマットがあっている
                    Ok(data) => Ok((data, line_header)),
                    // フォーマットが違っている場合
                    Err(_err) => match serde_json::from_value::<ErrorResponse>(json.clone()) {
                        Ok(error_response) => {
                            Err(Error::Line(error_response, status_code, line_header))
                        }
                        Err(_) => Err(Error::OtherJson(json, status_code, line_header)),
                    },
                };
                break;
            }
            Err(err) => {
                tracing::debug!("error: {:?}", err);

                // ステータスコードによってはリトライを行わない
                if !is_retry(
                    err.status_code()
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                ) {
                    // リトライしない
                    res = Err(*err);
                    break;
                }

                if i + 1 >= try_count {
                    // リトライ回数がオーバーしたので失敗にする
                    res = Err(*err);
                } else if !retry_duration.is_zero() {
                    // リトライ間隔がある場合は待つ
                    tokio::time::sleep(calc_retry_duration(retry_duration, i as u32, &mut rng))
                        .await;
                }
            }
        }
    }
    res.map_err(Box::new)
}
