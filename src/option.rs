//! `LineOptions`(API 呼び出しごとの設定)と、ログコールバックへ渡る
//! `LineRequestLog` / `LineResponseLog`、およびそれらの秘匿情報マスク処理を提供する。
//!
//! 公開型(`LineOptions` / `LineRequestLog` / `LineResponseLog` / `ResponseBody` /
//! `OnRequest` / `OnResponse` / `REDACTED_BODY_KEYS`)はクレートルート(`lib.rs`)からも
//! 再エクスポートされており、`lineapi::LineOptions` のように従来どおりのパスで参照できる。

use std::borrow::Cow;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use reqwest::{
    Method, StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};

/// API のベース URL のデフォルト。
const PREFIX_URL: &str = "https://api.line.me";
/// ベース URL を上書きする環境変数名。
const ENV_KEY: &str = "LINE_API_PREFIX_URL";

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
/// クエリ文字列に秘匿情報を載せるエンドポイント(GET verify の `access_token`)もあるため、
/// クエリは [`query`](Self::query) で生値が、[`query_redacted`](Self::query_redacted) で
/// 既知の秘匿キー(`access_token` 等)をマスクした値が得られる。GET verify の `access_token`
/// は既定の [`REDACTED_BODY_KEYS`] に含まれるため [`query_redacted`](Self::query_redacted) で
/// マスクされる。ログ出力時は各 `*_redacted` ヘルパーを使うこと。
#[derive(Clone)]
pub struct LineRequestLog<'a> {
    /// 送信直前にリクエストから捕捉した観測情報。`try_clone` / `build` 失敗時は `None`。
    ///
    /// `headers` / `method` / `path` は単一の `Option` 配下にまとめてあるため、これらは
    /// **必ず同運命**(全部 `Some` か全部 `None`)であり、「method はあるが path は無い」のような
    /// 不整合な状態は表現できない。クエリの有無は [`CapturedRequest::query`] の内側 `Option` で
    /// 別途表す(捕捉成功でもクエリ文字列が無ければ `None`)。
    captured: Option<&'a CapturedRequest>,
    body: &'a serde_json::Value,
    /// マスク対象のボディキー(`LineOptions` の設定値、未設定時は [`REDACTED_BODY_KEYS`])。
    redacted_body_keys: &'a [String],
}

/// 送信直前のリクエストから捕捉した観測情報(`headers` / `method` / `path` / `query`)。
///
/// `try_clone` -> `build` で得た 1 つの `reqwest::Request` から複製するため、これらは互いに
/// 整合している(同じ試行・同じリトライキー状態)。[`LineRequestLog`] は本体を
/// `Option<&CapturedRequest>` で保持し、捕捉失敗(`None`)と捕捉成功(`Some`)を 1 つの
/// `Option` で表すことで「一部だけ捕捉できた」不整合状態を表現不能にしている。
#[derive(Clone)]
pub(crate) struct CapturedRequest {
    /// リクエストヘッダー。
    pub(crate) headers: HeaderMap,
    /// HTTP メソッド(`GET` / `POST` など)。
    pub(crate) method: Method,
    /// リクエスト URL のパス(クエリ文字列は含まない)。
    pub(crate) path: String,
    /// リクエスト URL の生クエリ文字列。クエリが無いリクエストでは `None`(捕捉失敗とは別概念)。
    pub(crate) query: Option<String>,
}

// Debug を導出せず手実装するのは意図的。導出すると `{:?}` / `tracing::*(?log)` で
// `Authorization` トークンや OAuth ボディの秘匿情報が生のまま出力されてしまう。
// マスク済みの値(`headers_redacted` / `body_redacted`)のみを表示する。
impl std::fmt::Debug for LineRequestLog<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineRequestLog")
            .field("method", &self.method())
            .field("path", &self.path())
            .field("query", &self.query_redacted())
            .field("headers", &self.headers_redacted())
            .field("body", &self.body_redacted())
            .finish()
    }
}

impl<'a> LineRequestLog<'a> {
    /// クレート内部からログ情報を組み立てる(フィールドは非公開のため `new` 経由で構築する)。
    ///
    /// `captured` は送信直前に捕捉した [`CapturedRequest`] への参照(捕捉失敗時は `None`)。
    pub(crate) fn new(
        captured: Option<&'a CapturedRequest>,
        body: &'a serde_json::Value,
        redacted_body_keys: &'a [String],
    ) -> Self {
        Self {
            captured,
            body,
            redacted_body_keys,
        }
    }

    /// HTTP メソッド(`GET` / `POST` など)。
    ///
    /// リクエストの複製(`try_clone`)や再構築に失敗した場合は `None`
    /// ([`headers`](Self::headers) と同じ捕捉契約)。
    pub fn method(&self) -> Option<&'a Method> {
        self.captured.map(|c| &c.method)
    }

    /// リクエスト URL のパス(例: `/v2/bot/message/push`)。
    ///
    /// リクエストの複製(`try_clone`)や再構築に失敗した場合は `None`
    /// ([`headers`](Self::headers) と同じ捕捉契約)。クエリ文字列は含まない
    /// ([`query`](Self::query) を参照)。
    pub fn path(&self) -> Option<&'a str> {
        self.captured.map(|c| c.path.as_str())
    }

    /// リクエスト URL の**生の**クエリ文字列(例: `from=20240101&to=20241231`)。
    ///
    /// `None` になるのは「クエリ文字列が無い」場合と「リクエストの捕捉に失敗した」場合の
    /// 両方。両者を区別したい場合は [`path`](Self::path) を併用する(`path()` が `Some` なら
    /// 捕捉は成功しており、`query()` が `None` なのは純粋にクエリが無いため)。
    ///
    /// **⚠ 秘匿情報に注意**: クエリに秘匿情報を載せるエンドポイント(GET verify の
    /// `access_token`)があるため、本メソッドの戻り値はマスクされていない生値である。ログ等へ
    /// 出力する前に [`query_redacted`](Self::query_redacted) でマスクすること。
    pub fn query(&self) -> Option<&'a str> {
        self.captured.and_then(|c| c.query.as_deref())
    }

    /// クエリ文字列([`query`](Self::query))の既知の秘匿キーを `***` に置換した複製を返す。
    ///
    /// マスク対象キーは [`LineOptions`] の設定値
    /// ([`with_redacted_body_keys`](LineOptionsBuilder::with_redacted_body_keys))、未設定時は
    /// [`REDACTED_BODY_KEYS`](`access_token` 等)。キー比較は大文字小文字を無視する。GET verify の
    /// `access_token` は既定キーに含まれるためマスクされる。クエリが無い/捕捉失敗時は `None`。
    ///
    /// # 限界(許可リスト方式)
    ///
    /// マスクは設定されたキーの**完全一致**(大文字小文字無視)のみで行う。リストに無いキーは
    /// マスクされず素通りする。本メソッドの戻り値を「すべての秘匿情報が除去済み」とみなさないこと。
    /// また、マスクの有無に関わらず内部の `redact_query` が `url::form_urlencoded` で再エンコード
    /// するため、**秘匿キーを含まないペアも表現が正規化され得る**(空白の `+`↔`%20` など)。生値の
    /// 忠実な再現が必要なら [`query`](Self::query) を使うこと。
    pub fn query_redacted(&self) -> Option<String> {
        self.query()
            .map(|q| redact_query(q, self.redacted_body_keys))
    }

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
        self.captured.map(|c| &c.headers)
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
        self.headers().map(redact_headers)
    }

    /// ボディ([`body`](Self::body))の既知の秘匿キーを `***` に置換した複製を返す。
    ///
    /// マスク対象キーは [`LineOptions`] の設定値
    /// ([`with_redacted_body_keys`](LineOptionsBuilder::with_redacted_body_keys))、未設定時は [`REDACTED_BODY_KEYS`]
    /// (`client_secret` / `access_token` / `refresh_token` / `code` / `code_verifier` /
    /// `id_token` / `userAccessToken`)。ネストしたオブジェクト/配列も再帰的に走査する。
    ///
    /// # 限界(許可リスト方式)
    ///
    /// マスクは設定されたキーの**完全一致**(大文字小文字無視)のみで行う。リストに無いキー、
    /// 例えば LINE が将来追加するフィールドや、レスポンス型の `#[serde(flatten)] extra` 経由で
    /// 流れ込む未知の秘匿フィールドは**マスクされず素通りする**。本メソッドの戻り値を「すべての
    /// 秘匿情報が除去済み」とみなさないこと。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(self.body, self.redacted_body_keys)
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
    /// マスク対象のボディキー(`LineOptions` の設定値、未設定時は [`REDACTED_BODY_KEYS`])。
    redacted_body_keys: &'a [String],
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
    /// クレート内部からログ情報を組み立てる(フィールドは非公開のため `new` 経由で構築する)。
    pub(crate) fn new(
        headers: &'a HeaderMap,
        body: ResponseBody,
        status_code: StatusCode,
        redacted_body_keys: &'a [String],
    ) -> Self {
        Self {
            headers,
            body,
            status_code,
            redacted_body_keys,
        }
    }

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
    /// マスク対象キーは [`LineOptions`] の設定値
    /// ([`with_redacted_body_keys`](LineOptionsBuilder::with_redacted_body_keys))、未設定時は [`REDACTED_BODY_KEYS`]。token
    /// レスポンスの `access_token` / `refresh_token` / `id_token` などをマスクする。
    ///
    /// # 限界(許可リスト方式)
    ///
    /// マスクは設定されたキーの**完全一致**(大文字小文字無視)のみで行う。リストに無いキー、
    /// 例えば LINE が将来追加するフィールドや、レスポンス型の `#[serde(flatten)] extra` 経由で
    /// 流れ込む未知の秘匿フィールドは**マスクされず素通りする**。本メソッドの戻り値を「すべての
    /// 秘匿情報が除去済み」とみなさないこと。
    pub fn body_redacted(&self) -> serde_json::Value {
        redact_body(&self.as_value(), self.redacted_body_keys)
    }
}

/// [`body_redacted`](LineRequestLog::body_redacted) /
/// [`body_redacted`](LineResponseLog::body_redacted) でマスクされるボディのキー。
///
/// エントリは**すべて小文字**で記載し、マッチは大文字小文字を無視して行う
/// (例: `userAccessToken` は `useraccesstoken` のエントリでマッチする)。
///
/// これらのキーはリクエスト/レスポンス双方のボディで再帰的にマスクされ、さらに
/// [`query_redacted`](LineRequestLog::query_redacted) によるクエリ文字列のマスクにも使われる
/// (security-first)。`code` のような汎用キーは、レスポンス側に正当な `code` フィールドが
/// あっても `***` に潰し得るが、秘匿情報(OAuth の認可コード)の漏洩回避を優先した意図的な挙動である。
///
/// # メンテナンス上の注意
///
/// クエリ文字列やボディに**新たな秘匿情報を載せるエンドポイントを追加する際は、同じ変更内で
/// そのキーを本リストへ追加すること**。マスクは許可リスト方式(完全一致)であり、リストに無い
/// 秘匿キーは `*_redacted` を通しても素通りする。`_redacted` という名前を「常に安全」と過信しない。
pub const REDACTED_BODY_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "client_secret",
    "code",
    "code_verifier",
    "id_token",
    "useraccesstoken",
];

/// [`REDACTED_BODY_KEYS`] を `Vec<String>` 化したデフォルトのマスクキー。
///
/// [`LineRequestLog`] / [`LineResponseLog`] のマスクキーは `&[String]` で統一的に扱う
/// (設定値も `Vec<String>`)。`LineOptions` でキーが未設定のとき、
/// [`get_redacted_body_keys`](LineOptions::get_redacted_body_keys) はこの `'static` な既定値を返す。
static DEFAULT_REDACTED_BODY_KEYS: LazyLock<Vec<String>> =
    LazyLock::new(|| REDACTED_BODY_KEYS.iter().map(|s| s.to_string()).collect());

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

/// ボディ JSON を再帰的に走査し、`keys` に該当するキーの値を `***` に置換した複製を返す
/// (キー比較は大文字小文字を無視)。`keys` は小文字で渡される前提
/// ([`with_redacted_body_keys`](LineOptionsBuilder::with_redacted_body_keys)が正規化、既定の
/// [`REDACTED_BODY_KEYS`] も全小文字)。
fn redact_body(value: &serde_json::Value, keys: &[String]) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, val)| {
                    if keys.contains(&key.to_ascii_lowercase()) {
                        (key.clone(), serde_json::Value::String(REDACTED.to_string()))
                    } else {
                        (key.clone(), redact_body(val, keys))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(|item| redact_body(item, keys)).collect())
        }
        other => other.clone(),
    }
}

/// 生のクエリ文字列を `key=value` ペアに分解し、`keys` に該当するキーの値を `***` に置換した
/// クエリ文字列を再構築して返す(キー比較は大文字小文字を無視)。`keys` は小文字で渡される前提
/// (既定の [`REDACTED_BODY_KEYS`] は全小文字、
/// [`with_redacted_body_keys`](LineOptionsBuilder::with_redacted_body_keys) も正規化する)。
///
/// パース/再エンコードは [`url::form_urlencoded`] を用いるため、`%` エスケープや `+`(空白)も
/// 正しく解釈される。再エンコードは**秘匿キーを含まないペアにも及ぶ**ため、マスクの有無に関わらず
/// 表現が正規化され得る(空白の `+`↔`%20`、`=` 無しの裸キー `foo` → `foo=` など)。生値の忠実な
/// 再現が必要なら呼び出し側は [`LineRequestLog::query`] を使うこと。
fn redact_query(query: &str, keys: &[String]) -> String {
    let pairs = url::form_urlencoded::parse(query.as_bytes()).map(|(key, value)| {
        if keys.contains(&key.to_ascii_lowercase()) {
            (key.into_owned(), REDACTED.to_string())
        } else {
            (key.into_owned(), value.into_owned())
        }
    });
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
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
pub(crate) fn run_log_callback(label: &str, f: impl FnOnce()) {
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
/// [`LineOptionsBuilder::with_on_request`] /
/// [`LineOptionsBuilder::with_on_response`] で実行時に設定すること。
///
/// インスタンスは [`LineOptions::builder`]([`LineOptionsBuilder`])経由で構築する。
/// 設定無しのデフォルトだけが欲しい場合は [`LineOptions::default`] でもよい。
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
    /// `body_redacted` でマスクするボディキー(未設定時は [`REDACTED_BODY_KEYS`])。
    /// キーは小文字に正規化して保持する。
    pub(crate) redacted_body_keys: Option<Vec<String>>,
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
            .field("redacted_body_keys", &self.redacted_body_keys)
            .finish()
    }
}

impl LineOptions {
    /// [`LineOptionsBuilder`] を生成する。
    ///
    /// `LineOptions` の設定は本ビルダー経由で行う(`with_*` セッターはビルダー側にある)。
    /// 何も設定しないデフォルトが欲しいだけなら [`LineOptions::default`] でもよい。
    ///
    /// ```
    /// use lineapi::LineOptions;
    /// use std::time::Duration;
    ///
    /// let options = LineOptions::builder()
    ///     .with_try_count(3)
    ///     .with_retry_duration(Duration::from_millis(100))
    ///     .build();
    /// ```
    pub fn builder() -> LineOptionsBuilder {
        LineOptionsBuilder::default()
    }

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
    /// [`LineOptionsBuilder::with_prefix_url`] 未設定時は環境変数 `LINE_API_PREFIX_URL`、
    /// それも無ければデフォルトの `https://api.line.me` を返す(`make_url` と同じ解決順)。
    pub fn get_prefix_url(&self) -> String {
        self.resolve_prefix_url()
    }

    /// `body_redacted` でマスクされるボディキーの実効値を返す。
    ///
    /// [`LineOptionsBuilder::with_redacted_body_keys`] 未設定時は既定の
    /// [`REDACTED_BODY_KEYS`] を(`'static` な内部表現で)返す。返るキーはすべて小文字で、
    /// マスク照合は大文字小文字を無視して行われる。
    pub fn get_redacted_body_keys(&self) -> &[String] {
        self.redacted_body_keys
            .as_deref()
            .unwrap_or_else(|| &DEFAULT_REDACTED_BODY_KEYS)
    }

    /// `prefix_url` の実効値を解決する(設定値 → 環境変数 → デフォルト)。
    pub(crate) fn resolve_prefix_url(&self) -> String {
        self.prefix_url
            .clone()
            .unwrap_or_else(|| std::env::var(ENV_KEY).unwrap_or_else(|_| PREFIX_URL.to_string()))
    }
}

/// [`LineOptions`] を組み立てるビルダー。
///
/// [`LineOptions::builder`] もしくは [`LineOptionsBuilder::default`] で生成し、各 `with_*`
/// セッターで設定したのち [`build`](Self::build) で [`LineOptions`] を得る。
///
/// # serde について
///
/// `on_request` / `on_response` コールバックはシリアライズ対象外であり、ビルダー自体も
/// serde を実装しない。コールバックは [`with_on_request`](Self::with_on_request) /
/// [`with_on_response`](Self::with_on_response) で実行時に設定すること。
#[derive(Default, Clone)]
pub struct LineOptionsBuilder {
    prefix_url: Option<String>,
    timeout_duration: Option<Duration>,
    try_count: Option<u8>,
    retry_duration: Option<Duration>,
    on_request: Option<OnRequest>,
    on_response: Option<OnResponse>,
    redacted_body_keys: Option<Vec<String>>,
}

// Debug を導出せず手実装するのは意図的。`on_request` / `on_response` はクロージャ
// (`Arc<dyn Fn>`)で Debug を持たないため、設定有無のみを表示する。
impl std::fmt::Debug for LineOptionsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineOptionsBuilder")
            .field("prefix_url", &self.prefix_url)
            .field("timeout_duration", &self.timeout_duration)
            .field("try_count", &self.try_count)
            .field("retry_duration", &self.retry_duration)
            .field("on_request", &self.on_request.as_ref().map(|_| "Fn"))
            .field("on_response", &self.on_response.as_ref().map(|_| "Fn"))
            .field("redacted_body_keys", &self.redacted_body_keys)
            .finish()
    }
}

impl LineOptionsBuilder {
    /// 空のビルダーを生成する([`LineOptionsBuilder::default`] と同じ)。
    pub fn new() -> Self {
        Self::default()
    }

    /// 設定を確定して [`LineOptions`] を生成する。
    pub fn build(self) -> LineOptions {
        LineOptions {
            prefix_url: self.prefix_url,
            timeout_duration: self.timeout_duration,
            try_count: self.try_count,
            retry_duration: self.retry_duration,
            on_request: self.on_request,
            on_response: self.on_response,
            redacted_body_keys: self.redacted_body_keys,
        }
    }

    /// API のベース URL を設定する。
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
    /// `0` を渡しても保存値はそのまま `0` だが、実行時に [`LineOptions::get_try_count`]
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

    /// `body_redacted` でマスクするボディキーを設定する。
    ///
    /// 未設定時は既定の [`REDACTED_BODY_KEYS`](`client_secret` / `access_token` /
    /// `refresh_token` / `code` / `code_verifier` / `id_token` / `userAccessToken`)が使われる。
    /// 本メソッドで指定すると既定キーは**完全に置き換えられる**(マージではない)。既定キーも
    /// 残したい場合は [`REDACTED_BODY_KEYS`] を含めて渡すこと。
    ///
    /// 渡したキーは ASCII 小文字へ正規化して保持し、マスク照合は大文字小文字を無視して行う
    /// (例: `"userAccessToken"` を渡しても JSON 上の `userAccessToken` にマッチする)。
    /// 空のイテレータを渡すとマスク対象が無くなる(`body_redacted` が素通しになる)点に注意。
    pub fn with_redacted_body_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.redacted_body_keys = Some(
            keys.into_iter()
                .map(|s| s.as_ref().to_ascii_lowercase())
                .collect(),
        );
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
        let out = redact_body(&input, &DEFAULT_REDACTED_BODY_KEYS);
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
        let out = redact_body(&input, &DEFAULT_REDACTED_BODY_KEYS);
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
        let redacted = redact_body(&value, &DEFAULT_REDACTED_BODY_KEYS);
        assert_eq!(redacted["userAccessToken"], "***");
    }

    // get_try_count は 0 を 1 に正規化する(0 のままだとリトライループが回らず
    // 不透明な Error::Invalid("fail loop") を返してしまうため)。
    #[test]
    fn test_get_try_count_normalizes_zero() {
        assert_eq!(LineOptions::default().get_try_count(), 1, "None は 1");
        assert_eq!(
            LineOptions::builder()
                .with_try_count(0)
                .build()
                .get_try_count(),
            1,
            "0 は 1 に正規化"
        );
        assert_eq!(
            LineOptions::builder()
                .with_try_count(3)
                .build()
                .get_try_count(),
            3,
            "それ以外はそのまま"
        );
        // 保存値は正規化しない(正規化は読み取り時のみ)。
        assert_eq!(
            LineOptions::builder().with_try_count(0).build().try_count,
            Some(0)
        );
    }

    // on_request / on_response は #[serde(skip)] のため、serialize -> deserialize で
    // コールバックは失われるが、他のフィールドは保持される。
    #[test]
    fn test_line_options_serde_round_trip_drops_callbacks() {
        let options = LineOptions::builder()
            .with_prefix_url("https://example.com")
            .with_try_count(3)
            .with_on_request(|_log| {})
            .with_on_response(|_req, _res| {})
            .build();
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

    // テスト用に CapturedRequest を組み立てるヘルパー(headers は空で十分)。
    fn make_captured(method: Method, path: &str, query: Option<&str>) -> CapturedRequest {
        CapturedRequest {
            headers: HeaderMap::new(),
            method,
            path: path.to_string(),
            query: query.map(|q| q.to_string()),
        }
    }

    // captured が None のとき全アクセサが None を返す
    // (「ヘッダーが空」ではなく「捕捉に失敗」を表す契約。headers/method/path は同運命)。
    #[test]
    fn test_request_log_headers_none_contract() {
        let body = serde_json::Value::Null;
        let log = LineRequestLog::new(None, &body, &DEFAULT_REDACTED_BODY_KEYS);
        assert!(log.headers().is_none());
        assert!(log.headers_redacted().is_none());
        // 捕捉失敗時は method / path / query も None(headers と同じ契約)。
        assert!(log.method().is_none());
        assert!(log.path().is_none());
        assert!(log.query().is_none());
        assert!(log.query_redacted().is_none());
    }

    // method / path / query を持つログのアクセサが捕捉値をそのまま返す。
    #[test]
    fn test_request_log_method_path_query_accessors() {
        let body = serde_json::Value::Null;
        let captured = make_captured(
            Method::GET,
            "/v2/bot/message/aggregation/list",
            Some("limit=5&start=abc"),
        );
        let log = LineRequestLog::new(Some(&captured), &body, &DEFAULT_REDACTED_BODY_KEYS);
        assert_eq!(log.method(), Some(&Method::GET));
        assert_eq!(log.path(), Some("/v2/bot/message/aggregation/list"));
        assert_eq!(log.query(), Some("limit=5&start=abc"));
        // 秘匿キーを含まないクエリは素通し(再エンコードで表現は正規化され得る)。
        assert_eq!(log.query_redacted().as_deref(), Some("limit=5&start=abc"));
    }

    // 捕捉成功でもクエリが無いリクエスト(POST 等)では query() / query_redacted() のみ None。
    // path()/method() は Some なので「クエリ無し」と「捕捉失敗」を区別できる。
    #[test]
    fn test_request_log_query_none_but_captured() {
        let body = serde_json::Value::Null;
        let captured = make_captured(Method::POST, "/v2/bot/message/push", None);
        let log = LineRequestLog::new(Some(&captured), &body, &DEFAULT_REDACTED_BODY_KEYS);
        assert_eq!(log.method(), Some(&Method::POST));
        assert_eq!(log.path(), Some("/v2/bot/message/push"));
        // クエリ無し: query は None だが捕捉は成功している(path が Some)。
        assert!(log.query().is_none());
        assert!(log.query_redacted().is_none());
    }

    // GET verify の access_token クエリは query_redacted でマスクされる(秘匿クエリの漏洩回避)。
    #[test]
    fn test_request_log_query_redacted_masks_access_token() {
        let body = serde_json::Value::Null;
        let captured = make_captured(
            Method::GET,
            "/oauth2/v2.1/verify",
            Some("access_token=super-secret&keep=v"),
        );
        let log = LineRequestLog::new(Some(&captured), &body, &DEFAULT_REDACTED_BODY_KEYS);
        // 生値は秘匿情報を含む
        assert_eq!(log.query(), Some("access_token=super-secret&keep=v"));
        // redacted は access_token をマスクし、非秘匿キーは残す。
        // `***` は form_urlencoded で `*` を percent-encode しないため、そのまま `access_token=***`。
        let redacted = log.query_redacted().unwrap();
        assert_eq!(redacted, "access_token=***&keep=v");
        assert!(
            !redacted.contains("super-secret"),
            "生トークンが残っている: {redacted}"
        );
    }

    // query_redacted() は per-call の redacted_body_keys フィールド(execute_api_raw が
    // options.get_redacted_body_keys() から配線)を使う。カスタムキーを LineRequestLog::new に
    // 渡すと、そのキーがマスクされ既定の access_token は素通しになる(replace セマンティクス)。
    // query_redacted が REDACTED_BODY_KEYS をハードコードする回帰を検知する(配線レイヤの固定)。
    #[test]
    fn test_request_log_query_redacted_uses_per_call_keys() {
        let body = serde_json::Value::Null;
        // redact_query は keys を小文字前提で完全一致照合する(option.rs の redact_query/redact_body
        // 参照)。本テストは with_redacted_body_keys を経由せず new に直接渡すため、その正規化は
        // 走らない。よってここでは自前で小文字に揃えて渡す必要がある。
        let custom_keys = ["mysecret".to_string()];
        let captured = make_captured(
            Method::GET,
            "/oauth2/v2.1/verify",
            Some("mysecret=s&access_token=at"),
        );
        let log = LineRequestLog::new(Some(&captured), &body, &custom_keys);
        // カスタムキーはマスクされ、既定キー access_token は置換セマンティクスでマスクされない。
        assert_eq!(
            log.query_redacted().as_deref(),
            Some("mysecret=***&access_token=at")
        );
    }

    // LineRequestLog の Debug 出力はクエリ秘匿情報をマスクする(query_redacted 経由)。
    // 将来 Debug 実装が query() の生値へ差し替わると `?log` で access_token が漏れるため、
    // その回帰を検知して秘匿契約を固定する。
    #[test]
    fn test_request_log_debug_masks_query_secret() {
        let body = serde_json::Value::Null;
        let captured = make_captured(
            Method::GET,
            "/oauth2/v2.1/verify",
            Some("access_token=super-secret&keep=v"),
        );
        let log = LineRequestLog::new(Some(&captured), &body, &DEFAULT_REDACTED_BODY_KEYS);
        let dbg = format!("{log:?}");
        // マスク済み表現が出ている
        assert!(dbg.contains("***"), "Debug にマスク表現が無い: {dbg}");
        // 生トークンは Debug 出力に残らない
        assert!(
            !dbg.contains("super-secret"),
            "生トークンが Debug 出力に漏れている: {dbg}"
        );
    }

    // redact_query: 重複キーは両方マスクされる(全ペアを再シリアライズするため)。
    #[test]
    fn test_redact_query_masks_duplicate_keys() {
        let out = redact_query("access_token=a&access_token=b", &DEFAULT_REDACTED_BODY_KEYS);
        assert_eq!(out, "access_token=***&access_token=***");
    }

    // redact_query: percent-encoding / `+`(空白)のラウンドトリップ。非秘匿値は復号され、
    // 再エンコードで空白は `+` に正規化される(`%20` 入力でも `+` 出力)。
    #[test]
    fn test_redact_query_roundtrip_encoding() {
        // `+` も `%20` も空白として解釈され、出力は `+` に正規化される。
        assert_eq!(redact_query("q=a+b", &DEFAULT_REDACTED_BODY_KEYS), "q=a+b");
        assert_eq!(
            redact_query("q=a%20b", &DEFAULT_REDACTED_BODY_KEYS),
            "q=a+b"
        );
    }

    // redact_query: カスタムキー集合ではそのキーがマスクされ、既定の access_token は素通しになる
    // (replace-not-merge セマンティクスがクエリでも効くことを固定)。
    #[test]
    fn test_redact_query_custom_keys() {
        let keys = vec!["mysecret".to_string()];
        let out = redact_query("mySecret=s&access_token=at", &keys);
        // 大文字小文字無視でカスタムキーをマスク
        assert_eq!(out, "mySecret=***&access_token=at");
    }

    // redact_query: 空文字列は空のまま(クエリ無しは None なのでここには来ないが防御的に固定)。
    #[test]
    fn test_redact_query_empty() {
        assert_eq!(redact_query("", &DEFAULT_REDACTED_BODY_KEYS), "");
    }

    // REDACTED_BODY_KEYS の汎用キー `code` は、レスポンス側の正当な `code` フィールドも
    // `***` に潰す(意図的な security-first 挙動)。仕様であることを固定する。
    #[test]
    fn test_redact_body_masks_generic_code_in_response() {
        let response = serde_json::json!({
            "message": "invalid request",
            "code": "40000",
        });
        let redacted = redact_body(&response, &DEFAULT_REDACTED_BODY_KEYS);
        assert_eq!(redacted["code"], "***", "汎用 code も意図的にマスクされる");
        assert_eq!(redacted["message"], "invalid request");
    }

    // get_redacted_body_keys のデフォルトは REDACTED_BODY_KEYS と一致する。
    #[test]
    fn test_get_redacted_body_keys_default_matches_const() {
        let keys = LineOptions::default();
        let keys = keys.get_redacted_body_keys();
        assert_eq!(keys.len(), REDACTED_BODY_KEYS.len());
        for k in REDACTED_BODY_KEYS {
            assert!(keys.iter().any(|x| x == k), "default に {k} が無い");
        }
    }

    // with_redacted_body_keys はデフォルトを完全に置き換える。指定したカスタムキーはマスクされ、
    // 既定キー(access_token 等)はマスクされなくなる。さらに大文字指定は小文字へ正規化され、
    // camelCase の JSON キーにもマッチする。
    #[test]
    fn test_with_redacted_body_keys_replaces_and_normalizes() {
        let options = LineOptions::builder()
            .with_redacted_body_keys(["mySecret", "apiKey"])
            .build();
        // 大文字小文字無視で照合できるよう小文字に正規化されている
        assert_eq!(
            options.get_redacted_body_keys(),
            &["mysecret".to_string(), "apikey".to_string()]
        );

        let input = serde_json::json!({
            "mySecret": "s",
            "apiKey": "k",
            "access_token": "at",
        });
        let out = redact_body(&input, options.get_redacted_body_keys());
        // カスタムキーはマスクされる(大文字小文字無視)
        assert_eq!(out["mySecret"], "***");
        assert_eq!(out["apiKey"], "***");
        // 既定キーは置き換えられたのでマスクされない
        assert_eq!(out["access_token"], "at");
    }

    // 空のキーを渡すとマスク対象が無くなる(body_redacted が素通しになる)。
    #[test]
    fn test_with_redacted_body_keys_empty_disables_masking() {
        let options = LineOptions::builder()
            .with_redacted_body_keys(Vec::<String>::new())
            .build();
        let input = serde_json::json!({ "access_token": "at" });
        let out = redact_body(&input, options.get_redacted_body_keys());
        assert_eq!(out["access_token"], "at", "空指定ならマスクされない");
    }
}
