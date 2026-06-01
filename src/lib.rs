use std::borrow::Cow;
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
#[derive(Clone)]
pub struct LineRequestLog<'a> {
    headers: Option<&'a HeaderMap>,
    body: &'a serde_json::Value,
}

// Debug を導出せず手実装するのは意図的。導出すると `{:?}` / `tracing::*(?log)` で
// `Authorization` トークンや OAuth ボディの秘匿情報が生のまま出力されてしまう。
// マスク済みの値(`headers_redacted` / `body_redacted`)のみを表示する。
impl std::fmt::Debug for LineRequestLog<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineRequestLog")
            .field("headers", &self.headers_redacted())
            .field("body", &self.body_redacted())
            .finish()
    }
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

    /// 各エンドポイントがログ用に渡す論理表現を JSON 化したもの。
    ///
    /// フォームエンコード系エンドポイント(OAuth の token / revoke / POST verify)では
    /// 実際の送信形式は `application/x-www-form-urlencoded` であり、この JSON 表現とは
    /// 異なる。ボディを持たない GET(GET verify など)は `Value::Null`、クエリ系の GET
    /// (`get_v2_bot_message_aggregation_list` / `get_v2_bot_insight_message_event_aggregation`)
    /// は query params を JSON オブジェクト化した値になる(HTTP ボディそのものではなく
    /// ログ表現である点に注意)。
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
    ///
    /// # 限界(許可リスト方式)
    ///
    /// マスクは [`REDACTED_BODY_KEYS`] の**既知キー完全一致**のみで行う。リストに無いキー、
    /// 例えば LINE が将来追加するフィールドや、レスポンス型の `#[serde(flatten)] extra` 経由で
    /// 流れ込む未知の秘匿フィールドは**マスクされず素通りする**。本メソッドの戻り値を「すべての
    /// 秘匿情報が除去済み」とみなさないこと。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(self.body)
    }
}

/// レスポンスボディの内部表現。
///
/// JSON としてパースできたか否かを**型**で表すことで、「JSON 扱いなのに中身は生テキスト」
/// のような不整合な状態を表現不能にする。コールバック側で分岐なしに `Value` として扱いたい
/// 場合は [`LineResponseLog::as_value`] を使う。
#[derive(Debug, Clone)]
pub enum ResponseBody {
    /// JSON としてパースできたボディ。
    Json(serde_json::Value),
    /// JSON としてパースできなかった生テキストのボディ。
    Raw(String),
}

/// レスポンス受信後にコールバックへ渡される情報。
///
/// ボディは [`ResponseBody`] enum(`Json` / `Raw`)で表現し、JSON としてパースできたかを
/// **型**で区別する。`body_was_json` が真なのに中身が生テキスト、といった不整合な状態は
/// 表現できない。ログ出力やシリアライズ側で分岐なしに `Value` として扱いたい場合は
/// [`as_value`](Self::as_value) を使うと、JSON はそのまま、非 JSON は `Value::String` で
/// 包んだ値が一律で得られる。
///
/// レスポンスヘッダーは秘匿情報を含まない前提のため、[`LineRequestLog`] と異なり
/// `headers_redacted` 相当のヘルパーは提供しない(本クレートが観測するレスポンスヘッダーに
/// `Authorization` のような秘匿ヘッダーは無い)。
#[derive(Clone)]
pub struct LineResponseLog<'a> {
    headers: &'a HeaderMap,
    body: ResponseBody,
    status_code: StatusCode,
}

// Debug を導出せず手実装するのは意図的。token レスポンスの `access_token` /
// `refresh_token` / `id_token` などボディに秘匿情報が入り得るため、`{:?}` 出力では
// マスク済みの値(`body_redacted`)を表示する。ヘッダーは秘匿情報を含まない前提
// (本クレートが観測するレスポンスヘッダーに `Authorization` 等は無い)のためそのまま。
impl std::fmt::Debug for LineResponseLog<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineResponseLog")
            .field("headers", &self.headers)
            .field("status_code", &self.status_code)
            .field("body", &self.body_redacted())
            .finish()
    }
}

impl<'a> LineResponseLog<'a> {
    /// レスポンスヘッダー。
    ///
    /// `on_response` コールバック設定時のみ複製・保持される(未設定時は空)。この
    /// アクセサは `on_response` 経路からのみ到達する。
    pub fn headers(&self) -> &'a HeaderMap {
        self.headers
    }

    /// レスポンスボディを `serde_json::Value` として観測する。
    ///
    /// JSON としてパースできた場合はその値を借用で([`Cow::Borrowed`])、できなかった場合は
    /// 生テキストを `Value::String` で包んだ値を所有で([`Cow::Owned`])返す。両者を区別したい
    /// 場合は [`body_was_json`](Self::body_was_json) を参照すること(JSON 文字列ボディも
    /// `Value::String` になるため、値の形だけでは区別できない)。
    pub fn as_value(&self) -> Cow<'_, serde_json::Value> {
        match &self.body {
            ResponseBody::Json(value) => Cow::Borrowed(value),
            ResponseBody::Raw(text) => Cow::Owned(serde_json::Value::String(text.clone())),
        }
    }

    /// レスポンスのステータスコード。
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// レスポンスボディが JSON としてパースできたかどうか。
    ///
    /// `false` の場合、[`as_value`](Self::as_value) は生テキストを `Value::String` で包んだ値。
    pub fn body_was_json(&self) -> bool {
        matches!(self.body, ResponseBody::Json(_))
    }

    /// ボディ([`as_value`](Self::as_value))の既知の秘匿キーを `***` に置換した複製を返す。
    ///
    /// マスク対象キーは [`REDACTED_BODY_KEYS`] を参照。token レスポンスの
    /// `access_token` / `refresh_token` / `id_token` などをマスクする。
    ///
    /// # 限界(許可リスト方式)
    ///
    /// マスクは [`REDACTED_BODY_KEYS`] の**既知キー完全一致**のみで行う。リストに無いキー、
    /// 例えば LINE が将来追加するフィールドや、レスポンス型の `#[serde(flatten)] extra` 経由で
    /// 流れ込む未知の秘匿フィールドは**マスクされず素通りする**。本メソッドの戻り値を「すべての
    /// 秘匿情報が除去済み」とみなさないこと。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(&self.as_value())
    }
}

/// [`body_redacted`](LineRequestLog::body_redacted) /
/// [`body_redacted`](LineResponseLog::body_redacted) でマスクされるボディのキー。
///
/// エントリは**すべて小文字**で記載し、マッチは大文字小文字を無視して行う
/// (例: `userAccessToken` は `useraccesstoken` のエントリでマッチする)。
///
/// これらのキーはリクエスト/レスポンス双方のボディで再帰的にマスクされる(security-first)。
/// `code` のような汎用キーは、レスポンス側に正当な `code` フィールドがあっても `***` に
/// 潰し得るが、秘匿情報(OAuth の認可コード)の漏洩回避を優先した意図的な挙動である。
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
///
/// マスク対象を `Authorization` のみに絞っているのは意図的である。本クレートが付与する
/// ヘッダーのうち秘匿情報は `Authorization: Bearer <token>` だけで、`X-Line-Retry-Key`
/// などその他のヘッダーは秘匿情報ではないため。
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
/// `{"_serialize_error": "<理由>"}` を返す。`_serialize_error` は番兵キーだが、LINE の
/// リクエスト型(`RequestBody` / `QueryParams`)に同名フィールドは存在しないため実ボディと
/// 衝突しない。そもそも対象は文字列キーの serde 構造体であり、シリアライズ失敗は実質
/// 発生しない防御的経路である。
pub(crate) fn serialize_log_body<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value)
        .unwrap_or_else(|err| serde_json::json!({ "_serialize_error": err.to_string() }))
}

/// ログコールバックを panic 隔離して実行する。
///
/// コールバックは利用側のコードであり、内部で panic し得る。ログは観測のための副経路で
/// あるべきなので、panic を捕捉してログ出力し、API 呼び出し自体は継続させる。
fn run_log_callback(label: &str, f: impl FnOnce()) {
    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        // panic ペイロード(通常は &str / String)を取り出してログに残す。content-free な
        // 「panicした」だけのログは事後調査の役に立たないため、メッセージ本体を保持する。
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        tracing::error!(
            callback = label,
            panic = %msg,
            "LineOptions callback panicked; ignored to keep the API call alive"
        );
    }
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
    pub(crate) prefix_url: Option<String>,
    pub(crate) timeout_duration: Option<Duration>,
    pub(crate) try_count: Option<u8>,
    pub(crate) retry_duration: Option<Duration>,
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
        // 0 は 1 に正規化する。0 のままだとリトライループが一度も回らず、
        // 不透明な `Error::Invalid("fail loop")` が返ってしまうため。
        self.try_count.unwrap_or(1).max(1)
    }

    pub fn get_retry_duration(&self) -> Duration {
        self.retry_duration.unwrap_or(Duration::from_secs(0))
    }

    pub fn get_timeout_duration(&self) -> Duration {
        self.timeout_duration.unwrap_or(Duration::from_secs(0))
    }

    /// 実際に使用されるベース URL を返す。
    ///
    /// [`with_prefix_url`](Self::with_prefix_url) 未設定時は環境変数 `LINE_API_PREFIX_URL`、
    /// それも無ければデフォルトの `https://api.line.me` を返す([`make_url`] と同じ解決順)。
    pub fn get_prefix_url(&self) -> String {
        self.resolve_prefix_url()
    }

    /// `prefix_url` の実効値を解決する(`with_prefix_url` → 環境変数 → デフォルト)。
    pub(crate) fn resolve_prefix_url(&self) -> String {
        self.prefix_url
            .clone()
            .unwrap_or_else(|| std::env::var(ENV_KEY).unwrap_or_else(|_| PREFIX_URL.to_string()))
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
    ///
    /// `0` を渡しても保存値はそのまま `0` だが、実行時に [`get_try_count`](Self::get_try_count)
    /// が最低 1 回として正規化する(設定時ではなく読み取り時の正規化)。
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
    /// **⚠ 秘匿情報に注意**: コールバックに渡される [`LineRequestLog`] は**マスクされていない
    /// 生の値**であり、`Authorization` ヘッダーや OAuth 系ボディの秘匿情報を含み得る。ログ等へ
    /// 出力する前に必ず [`headers_redacted`](LineRequestLog::headers_redacted) /
    /// [`body_redacted`](LineRequestLog::body_redacted) でマスクすること(詳細は
    /// [`LineRequestLog`] のドキュメント参照)。
    ///
    /// コールバックは API 呼び出しのリクエスト経路で同期的に実行される。リトライ時は**試行
    /// ごと**に発火する(論理的な 1 回の呼び出しでも `try_count` 回呼ばれ得る)。内部で panic
    /// した場合は捕捉してログ出力し、API 呼び出し自体は継続する(ログは観測の副経路に徹する)。
    /// ただしコールバックが共有ロックを保持したまま panic すると、そのロックは poison され
    /// 得る(panic 捕捉では巻き戻せない)ため、引き続き panic させないことを推奨する。
    pub fn with_on_request(mut self, f: impl Fn(&LineRequestLog) + Send + Sync + 'static) -> Self {
        self.on_request = Some(Arc::new(f));
        self
    }

    /// レスポンス受信後に呼ばれるコールバックを設定する。
    ///
    /// **⚠ 秘匿情報に注意**: コールバックに渡される [`LineRequestLog`] / [`LineResponseLog`]
    /// は**マスクされていない生の値**であり、`Authorization` ヘッダーや OAuth 系のボディ/
    /// レスポンス(token レスポンスの `access_token` 等)の秘匿情報を含み得る。ログ等へ出力
    /// する前に必ず各 `*_redacted` ヘルパーでマスクすること(詳細は [`LineRequestLog`] /
    /// [`LineResponseLog`] のドキュメント参照)。
    ///
    /// コールバックは API 呼び出しのレスポンス経路で同期的に実行される。リトライ時は**試行
    /// ごと**に発火する(論理的な 1 回の呼び出しでも `try_count` 回呼ばれ得る)。内部で panic
    /// した場合は捕捉してログ出力し、API 呼び出し自体は継続する(ログは観測の副経路に徹する)。
    /// ただしコールバックが共有ロックを保持したまま panic すると、そのロックは poison され
    /// 得る(panic 捕捉では巻き戻せない)ため、引き続き panic させないことを推奨する。
    pub fn with_on_response(
        mut self,
        f: impl Fn(&LineRequestLog, &LineResponseLog) + Send + Sync + 'static,
    ) -> Self {
        self.on_response = Some(Arc::new(f));
        self
    }
}

pub(crate) fn make_url(postfix_url: &str, options: &LineOptions) -> String {
    format!("{}{postfix_url}", options.resolve_prefix_url())
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
    // ヘッダーが存在するのに非 ASCII 等で to_str() に失敗した場合は、空文字に潰す前に
    // warn を出す。サポート照会で最重要の request id が「欠落」と「パース失敗」で
    // 区別できないまま無言で空になるのを避ける(値自体は従来通り空文字)。
    let request_id = headers
        .get("X-Line-Request-Id")
        .map(|it| {
            it.to_str().unwrap_or_else(|_| {
                tracing::warn!("X-Line-Request-Id present but not valid ASCII; recording empty");
                ""
            })
        })
        .unwrap_or("");
    let accepted_request_id = headers.get("X-Line-Accepted-Request-Id").map(|it| {
        it.to_str()
            .unwrap_or_else(|_| {
                tracing::warn!(
                    "X-Line-Accepted-Request-Id present but not valid ASCII; recording empty"
                );
                ""
            })
            .to_string()
    });
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
    // Jitter
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
        run_log_callback("on_request", || {
            cb(&LineRequestLog {
                headers: request_headers.as_ref(),
                body: request_value,
            });
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
        // JSONならパース結果、非JSONなら生テキストを ResponseBody enum で渡す
        let response_body = match json_result.as_ref() {
            Ok(value) => ResponseBody::Json(value.clone()),
            Err(_) => ResponseBody::Raw(text.clone()),
        };
        run_log_callback("on_response", || {
            cb(
                &LineRequestLog {
                    headers: request_headers.as_ref(),
                    body: request_value,
                },
                &LineResponseLog {
                    headers: &response_headers,
                    body: response_body,
                    status_code,
                },
            );
        });
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

    // 実際の deauthorize リクエストボディ(ヘッダーとボディの双方に秘匿情報を持つ唯一の
    // エンドポイント)を、フィールド名込みで serialize -> redact に通し、camelCase の
    // `userAccessToken` が確実にマスクされることを固定する。
    #[test]
    fn test_redact_deauthorize_request_body() {
        use crate::line_login::post_user_v1_deauthorize::RequestBody;
        let body = RequestBody {
            user_access_token: "super-secret-token".to_string(),
        };
        let value = serialize_log_body(&body);
        // serde rename により JSON 上は camelCase になる
        assert_eq!(value["userAccessToken"], "super-secret-token");
        let redacted = redact_body(&value);
        assert_eq!(redacted["userAccessToken"], "***");
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

    // コールバックが panic しても API 呼び出しは成功する(ログは副経路に徹し、panic は
    // run_log_callback で捕捉される)。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_callback_panic_does_not_fail_api() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/test")
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default()
            .with_on_request(|_log| panic!("on_request callback panics"))
            .with_on_response(|_req, _res| panic!("on_response callback panics"));

        let result: Result<(serde_json::Value, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().get(&url),
            &options,
            is_standard_retry,
            None,
            || serde_json::Value::Null,
        )
        .await;

        assert!(result.is_ok(), "callback panic must not fail the API call");
        mock.assert_async().await;
    }

    // get_try_count は 0 を 1 に正規化する(0 のままだとリトライループが回らず
    // 不透明な Error::Invalid("fail loop") を返してしまうため)。
    #[test]
    fn test_get_try_count_normalizes_zero() {
        assert_eq!(LineOptions::default().get_try_count(), 1, "None は 1");
        assert_eq!(
            LineOptions::default().with_try_count(0).get_try_count(),
            1,
            "0 は 1 に正規化"
        );
        assert_eq!(
            LineOptions::default().with_try_count(3).get_try_count(),
            3,
            "それ以外はそのまま"
        );
        // 保存値は正規化しない(正規化は読み取り時のみ)。
        assert_eq!(LineOptions::default().with_try_count(0).try_count, Some(0));
    }

    // on_request / on_response は #[serde(skip)] のため、serialize -> deserialize で
    // コールバックは失われるが、他のフィールドは保持される。
    #[test]
    fn test_line_options_serde_round_trip_drops_callbacks() {
        let options = LineOptions::default()
            .with_prefix_url("https://example.com")
            .with_try_count(3)
            .with_on_request(|_log| {})
            .with_on_response(|_req, _res| {});
        assert!(options.on_request.is_some());

        let json = serde_json::to_string(&options).unwrap();
        let restored: LineOptions = serde_json::from_str(&json).unwrap();

        // 設定フィールドは保持される
        assert_eq!(restored.prefix_url.as_deref(), Some("https://example.com"));
        assert_eq!(restored.try_count, Some(3));
        // コールバックは落ちる
        assert!(restored.on_request.is_none());
        assert!(restored.on_response.is_none());
    }

    // headers が None のとき headers()/headers_redacted() は共に None を返す
    // (「ヘッダーが空」ではなく「捕捉に失敗」を表す契約)。
    #[test]
    fn test_request_log_headers_none_contract() {
        let body = serde_json::Value::Null;
        let log = LineRequestLog {
            headers: None,
            body: &body,
        };
        assert!(log.headers().is_none());
        assert!(log.headers_redacted().is_none());
    }

    // REDACTED_BODY_KEYS の汎用キー `code` は、レスポンス側の正当な `code` フィールドも
    // `***` に潰す(意図的な security-first 挙動)。仕様であることを固定する。
    #[test]
    fn test_redact_body_masks_generic_code_in_response() {
        let response = serde_json::json!({
            "message": "invalid request",
            "code": "40000",
        });
        let redacted = redact_body(&response);
        assert_eq!(redacted["code"], "***", "汎用 code も意図的にマスクされる");
        assert_eq!(redacted["message"], "invalid request");
    }

    // 補足: レスポンスボディの読取失敗(`response.text()` のエラー)が Error::Reqwest として
    // 伝播する経路(execute_api_raw)は、mockito では決定的に途中切断を起こしにくいため
    // ユニットテスト化していない。コード上は `.map_err(Error::Reqwest)?` で握り潰さず伝播する。
}
