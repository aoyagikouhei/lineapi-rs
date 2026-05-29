use std::sync::Arc;
use std::time::Duration;

use rand::{RngExt, rngs::StdRng};
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{self, AUTHORIZATION, HeaderMap, HeaderValue},
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

/// 秘匿情報をマスクする際の置換文字列。
const REDACTED: &str = "***";

/// リクエスト送信直前にコールバックへ渡される情報。
///
/// # 秘匿情報の扱い
///
/// 秘匿情報は**ヘッダー側**と**ボディ側**の双方に入り得る。
///
/// - ヘッダー: メッセージング系および一部のログイン系
///   (例: `post_user_v1_deauthorize`)は [`headers`](Self::headers) に
///   `Authorization: Bearer <token>` を含む。
/// - ボディ: OAuth ログイン系(token / revoke / POST verify / deauthorize)は
///   `client_secret` / `refresh_token` / `access_token` / `code` などの秘匿情報を
///   [`body`](Self::body) 側に持つ(`post_user_v1_deauthorize` はヘッダーとボディの
///   両方に秘匿情報を持つ)。
///
/// ログ出力時は [`headers_redacted`](Self::headers_redacted) でヘッダーを、
/// [`body_redacted`](Self::body_redacted) でボディの既知の秘匿キーをマスクできる。
///
/// なお、クエリ文字列に秘匿情報を載せるエンドポイント(GET verify の `access_token`)は
/// コールバックへ渡されない([`headers`](Self::headers) にも [`body`](Self::body) にも
/// 現れない)ため、呼び出し側でマスクする対象は存在しない。
#[derive(Debug, Clone)]
pub struct LineRequestLog<'a> {
    headers: Option<&'a HeaderMap>,
    body: &'a serde_json::Value,
}

impl<'a> LineRequestLog<'a> {
    /// リクエストヘッダー。
    ///
    /// リクエストの複製(`try_clone`)や再構築に失敗した場合は `None`。これにより
    /// 「ヘッダーが空」と「捕捉に失敗」を区別できる。なお現行の全エンドポイントは
    /// in-memory なボディ(`.json` / `.form` / `.query`)を使うため `try_clone` は
    /// 失敗せず、通常 `None` にはならない。
    ///
    /// 捕捉したヘッダーは `RequestBuilder` を再構築した時点のもので、送信時に reqwest が
    /// 付与する `content-length` / `host` などは含まれない。
    pub fn headers(&self) -> Option<&'a HeaderMap> {
        self.headers
    }

    /// リクエスト内容を JSON 化した論理表現。
    ///
    /// フォームエンコード系エンドポイント(OAuth の token / revoke / POST verify)では
    /// 実際の送信形式は `application/x-www-form-urlencoded` であり、この JSON 表現とは
    /// 異なる。GET 系(GET verify を含む)はボディを持たないため `Value::Null` になる。
    /// シリアライズに失敗した場合は `{"_serialize_error": "<理由>"}` となり、`Value::Null`
    /// (=ボディ無し)とは区別できる。
    pub fn body(&self) -> &'a serde_json::Value {
        self.body
    }

    /// `Authorization` ヘッダー値を `***` に置換したヘッダーの複製を返す。
    ///
    /// ヘッダーのみをマスクする。OAuth 系のようにボディへ秘匿情報が入る場合は
    /// [`body_redacted`](Self::body_redacted) を使うこと。捕捉失敗時は `None`。
    pub fn headers_redacted(&self) -> Option<HeaderMap> {
        self.headers.map(redact_headers)
    }

    /// ボディ([`body`](Self::body))の既知の秘匿キーを `***` に置換した複製を返す。
    ///
    /// マスク対象キーは [`REDACTED_BODY_KEYS`] を参照(`client_secret` /
    /// `access_token` / `refresh_token` / `code` / `code_verifier` / `id_token` /
    /// `userAccessToken`)。ネストしたオブジェクト/配列も再帰的に走査する。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(self.body)
    }
}

/// レスポンス受信後にコールバックへ渡される情報。
#[derive(Debug, Clone)]
pub struct LineResponseLog<'a> {
    headers: &'a HeaderMap,
    body: &'a serde_json::Value,
    status_code: StatusCode,
    body_was_json: bool,
}

impl<'a> LineResponseLog<'a> {
    /// レスポンスヘッダー。
    pub fn headers(&self) -> &'a HeaderMap {
        self.headers
    }

    /// レスポンスボディ。
    ///
    /// JSON としてパースできた場合はその値、できなかった場合は生テキストを
    /// `Value::String` で包んだ値が渡される。両者を区別したい場合は
    /// [`body_was_json`](Self::body_was_json) を参照すること(JSON 文字列ボディも
    /// `Value::String` になるため、値の形だけでは区別できない)。
    pub fn body(&self) -> &'a serde_json::Value {
        self.body
    }

    /// レスポンスのステータスコード。
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// レスポンスボディが JSON としてパースできたかどうか。
    ///
    /// `false` の場合、[`body`](Self::body) は生テキストを `Value::String` で包んだ値。
    pub fn body_was_json(&self) -> bool {
        self.body_was_json
    }

    /// ボディ([`body`](Self::body))の既知の秘匿キーを `***` に置換した複製を返す。
    ///
    /// マスク対象キーは [`REDACTED_BODY_KEYS`] を参照。token レスポンスの
    /// `access_token` / `refresh_token` / `id_token` などをマスクする。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(self.body)
    }
}

/// [`body_redacted`](LineRequestLog::body_redacted) /
/// [`body_redacted`](LineResponseLog::body_redacted) でマスクされるボディのキー。
///
/// マッチは大文字小文字を無視して行う。
pub const REDACTED_BODY_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "client_secret",
    "code",
    "code_verifier",
    "id_token",
    "useraccesstoken",
];

/// `Authorization` ヘッダー値を `***` に置換したヘッダーの複製を返す。
fn redact_headers(headers: &HeaderMap) -> HeaderMap {
    let mut redacted = headers.clone();
    if redacted.contains_key(AUTHORIZATION) {
        redacted.insert(AUTHORIZATION, HeaderValue::from_static(REDACTED));
    }
    redacted
}

/// ボディ JSON を再帰的に走査し、[`REDACTED_BODY_KEYS`] に該当するキーの値を `***` に
/// 置換した複製を返す(キー比較は大文字小文字を無視)。
fn redact_body(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, val)| {
                    if REDACTED_BODY_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                        (key.clone(), serde_json::Value::String(REDACTED.to_string()))
                    } else {
                        (key.clone(), redact_body(val))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_body).collect())
        }
        other => other.clone(),
    }
}

/// ログ用にリクエストボディを JSON 化する。
///
/// シリアライズに失敗した場合は `Value::Null`(=ボディ無し)と区別できるよう
/// `{"_serialize_error": "<理由>"}` を返す。
pub(crate) fn serialize_log_body<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value)
        .unwrap_or_else(|err| serde_json::json!({ "_serialize_error": err.to_string() }))
}

/// リクエスト送信直前に呼ばれるコールバック。
pub type OnRequest = Arc<dyn Fn(&LineRequestLog) + Send + Sync>;

/// レスポンス受信後に呼ばれるコールバック。
pub type OnResponse = Arc<dyn Fn(&LineRequestLog, &LineResponseLog) + Send + Sync>;

/// API 呼び出しごとの設定。
///
/// # serde について
///
/// `on_request` / `on_response` コールバックは `#[serde(skip)]` 指定のため
/// **シリアライズ/デシリアライズの対象外**。設定済みの `LineOptions` を serialize →
/// deserialize するとコールバックは失われる(`None` になる)。コールバックは
/// [`with_on_request`](Self::with_on_request) /
/// [`with_on_response`](Self::with_on_response) で実行時に設定すること。
#[derive(Default, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LineOptions {
    pub prefix_url: Option<String>,
    pub timeout_duration: Option<Duration>,
    pub try_count: Option<u8>,
    pub retry_duration: Option<Duration>,
    /// リクエスト送信直前に呼ばれるコールバック(指定時のみ)。
    #[serde(skip)]
    pub(crate) on_request: Option<OnRequest>,
    /// レスポンス受信後に呼ばれるコールバック(指定時のみ)。
    #[serde(skip)]
    pub(crate) on_response: Option<OnResponse>,
}

impl std::fmt::Debug for LineOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineOptions")
            .field("prefix_url", &self.prefix_url)
            .field("timeout_duration", &self.timeout_duration)
            .field("try_count", &self.try_count)
            .field("retry_duration", &self.retry_duration)
            .field("on_request", &self.on_request.as_ref().map(|_| "Fn"))
            .field("on_response", &self.on_response.as_ref().map(|_| "Fn"))
            .finish()
    }
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

    /// API のベース URL を設定する。
    ///
    /// `LineOptions` は `#[non_exhaustive]` のため、外部クレートからは構造体リテラルでは
    /// 構築できない。`LineOptions::default()` と各 `with_*` メソッドを併用すること。
    pub fn with_prefix_url(mut self, prefix_url: impl Into<String>) -> Self {
        self.prefix_url = Some(prefix_url.into());
        self
    }

    /// リクエストのタイムアウトを設定する。
    pub fn with_timeout_duration(mut self, timeout_duration: Duration) -> Self {
        self.timeout_duration = Some(timeout_duration);
        self
    }

    /// 試行回数(リトライ含む)を設定する。
    pub fn with_try_count(mut self, try_count: u8) -> Self {
        self.try_count = Some(try_count);
        self
    }

    /// リトライ間隔の基準値を設定する。
    pub fn with_retry_duration(mut self, retry_duration: Duration) -> Self {
        self.retry_duration = Some(retry_duration);
        self
    }

    /// リクエスト送信直前に呼ばれるコールバックを設定する。
    ///
    /// 渡される [`LineRequestLog`] には秘匿情報(`Authorization` ヘッダーや OAuth 系の
    /// ボディ)が含まれ得る。ログ出力時のマスクについては [`LineRequestLog`] の
    /// ドキュメントを参照。
    ///
    /// コールバックは API 呼び出しのリクエスト経路で同期的に実行される。内部で panic
    /// すると API 呼び出し自体が失敗し、コールバックが共有するロック等を poison し得る
    /// ため、panic させないこと。
    pub fn with_on_request(mut self, f: impl Fn(&LineRequestLog) + Send + Sync + 'static) -> Self {
        self.on_request = Some(Arc::new(f));
        self
    }

    /// レスポンス受信後に呼ばれるコールバックを設定する。
    ///
    /// 渡される [`LineRequestLog`] / [`LineResponseLog`] には秘匿情報(`Authorization`
    /// ヘッダーや OAuth 系のボディ/レスポンス)が含まれ得る。ログ出力時のマスクについては
    /// [`LineRequestLog`] / [`LineResponseLog`] のドキュメントを参照。
    ///
    /// コールバックは API 呼び出しのレスポンス経路で同期的に実行される。内部で panic
    /// すると API 呼び出し自体が失敗し、コールバックが共有するロック等を poison し得る
    /// ため、panic させないこと。
    pub fn with_on_response(
        mut self,
        f: impl Fn(&LineRequestLog, &LineResponseLog) + Send + Sync + 'static,
    ) -> Self {
        self.on_response = Some(Arc::new(f));
        self
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
    options: &LineOptions,
    request_value: &serde_json::Value,
) -> Result<(serde_json::Value, LineResponseHeader, StatusCode), Box<Error>> {
    let need_log = options.on_request.is_some() || options.on_response.is_some();

    // リクエストヘッダーを取得(コールバック設定時のみ)。
    // try_clone -> build で Request を得て headers を clone する。
    // リトライキー付与後の builder を受け取るので X-Line-Retry-Key も含まれる。
    // try_clone / build に失敗した場合は None とし、捕捉失敗を呼び出し側へ伝える。
    let request_headers: Option<HeaderMap> = if need_log {
        builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .map(|req| req.headers().clone())
    } else {
        None
    };

    if let Some(cb) = &options.on_request {
        cb(&LineRequestLog {
            headers: request_headers.as_ref(),
            body: request_value,
        });
    }

    let response = builder
        .send()
        .await
        .map_err(|err| Box::new(Error::Reqwest(err)))?;
    let status_code = response.status();
    let line_header = make_line_header(&response);
    let response_headers = if options.on_response.is_some() {
        response.headers().clone()
    } else {
        HeaderMap::new()
    };
    // ボディ読取失敗は握り潰さず伝播する(読めなかったボディは観測経路にも乗せない)。
    let text = response
        .text()
        .await
        .map_err(|err| Box::new(Error::Reqwest(err)))?;
    let json_result = serde_json::from_str::<serde_json::Value>(&text);

    if let Some(cb) = &options.on_response {
        // JSONならパース結果、非JSONなら生テキストを文字列Valueで渡す
        let response_value = json_result
            .as_ref()
            .ok()
            .cloned()
            .unwrap_or_else(|| serde_json::Value::String(text.clone()));
        cb(
            &LineRequestLog {
                headers: request_headers.as_ref(),
                body: request_value,
            },
            &LineResponseLog {
                headers: &response_headers,
                body: &response_value,
                status_code,
                body_was_json: json_result.is_ok(),
            },
        );
    }

    let Ok(json) = json_result else {
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
    request_value_fn: impl FnOnce() -> serde_json::Value,
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
    // コールバック設定時のみ request body をシリアライズする(未設定時の無駄を避ける)。
    let request_value = if options.on_request.is_some() || options.on_response.is_some() {
        request_value_fn()
    } else {
        serde_json::Value::Null
    };
    let mut rng: StdRng = rand::make_rng();
    for i in 0..try_count {
        // リクエスト準備
        let mut builder = f();
        // リトライキー付与は try_count が 2 以上(リトライあり)の場合のみ
        if let Some(retry_key) = &retry_key
            && try_count > 1
        {
            // リトライ回数がある場合はリトライキーをヘッダーに追加
            builder = builder.header(HEADER_RETRY_KEY, retry_key);
        }
        match execute_api_raw(builder, retry_key.is_some(), options, &request_value).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_log_body_success() {
        let value = serialize_log_body(&serde_json::json!({"a": 1}));
        assert_eq!(value, serde_json::json!({"a": 1}));
    }

    #[test]
    fn test_serialize_log_body_failure_sentinel() {
        use std::collections::HashMap;
        // 非文字列キーの map は JSON 化に失敗するため sentinel が返る
        let mut map: HashMap<Vec<i32>, i32> = HashMap::new();
        map.insert(vec![1, 2], 3);
        let value = serialize_log_body(&map);
        assert!(
            value.get("_serialize_error").is_some(),
            "expected serialize error sentinel, got: {value}"
        );
        // ボディ無し(Null)とは区別できる
        assert_ne!(value, serde_json::Value::Null);
    }

    #[test]
    fn test_redact_body_masks_known_keys_recursively() {
        let input = serde_json::json!({
            "client_secret": "secret",
            "grant_type": "authorization_code",
            "nested": { "refresh_token": "rt", "keep": "v" },
            "list": [ { "access_token": "at" } ],
        });
        let out = redact_body(&input);
        assert_eq!(out["client_secret"], "***");
        assert_eq!(out["grant_type"], "authorization_code");
        assert_eq!(out["nested"]["refresh_token"], "***");
        assert_eq!(out["nested"]["keep"], "v");
        assert_eq!(out["list"][0]["access_token"], "***");
    }

    #[test]
    fn test_redact_body_case_insensitive() {
        // deauthorize の userAccessToken(camelCase)もマスクされる
        let input = serde_json::json!({ "userAccessToken": "x", "ID_TOKEN": "y" });
        let out = redact_body(&input);
        assert_eq!(out["userAccessToken"], "***");
        assert_eq!(out["ID_TOKEN"], "***");
    }

    // コールバック未設定なら request_value_fn は呼ばれない(無駄なシリアライズを避ける)。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_no_callback_skips_request_value_fn() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/test")
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default();

        // コールバック未設定なので、呼ばれたら panic するクロージャでも問題なく完了する
        let result: Result<(serde_json::Value, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().get(&url),
            &options,
            is_standard_retry,
            None,
            || panic!("request_value_fn must not be called when no callback is set"),
        )
        .await;

        assert!(result.is_ok());
        mock.assert_async().await;
    }
}
