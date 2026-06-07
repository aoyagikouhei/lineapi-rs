use std::time::Duration;

use rand::{RngExt, rngs::StdRng};
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{self, AUTHORIZATION, HeaderMap},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::{Error, ErrorResponse, LineLoginErrorResponse};

pub mod error;
pub mod line_login;
pub mod messaging_api;
pub mod option;

#[cfg(feature = "mock")]
pub mod mock;

// `LineOptions` / ログ関連型は `option` モジュールへ移動した。クレートルートからも
// 従来どおりのパス(`lineapi::LineOptions` 等)で参照できるよう再エクスポートする。
pub use option::{
    LineOptions, LineOptionsBuilder, LineRequestLog, LineResponseLog, OnRequest, OnResponse,
    REDACTED_BODY_KEYS, ResponseBody,
};
// クレート内部で使うログヘルパー。`crate::serialize_log_body` 等の従来パスを維持する。
pub(crate) use option::{CapturedRequest, run_log_callback, serialize_log_body};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LineResponseHeader {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_request_id: Option<String>,
}

// 以前ここにあった `LineRequestLog` / `LineResponseLog` / `ResponseBody` /
// `LineOptions` / 秘匿マスク処理は `src/option.rs` へ移動した。

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

    // リクエストの観測情報(headers / method / path / query)を取得(コールバック設定時のみ)。
    // try_clone -> build で Request を得て、同じ Request から `CapturedRequest` にまとめて複製する。
    // リトライキー付与後の builder を受け取るので X-Line-Retry-Key も含まれる。
    // try_clone / build に失敗した場合は None とし、捕捉失敗を呼び出し側へ伝える(headers/method/path
    // は単一 Option で同運命。query はクエリ文字列が無いリクエストでは捕捉成功でも内側 None になる)。
    let captured: Option<CapturedRequest> = if need_log {
        builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .map(|req| CapturedRequest {
                headers: req.headers().clone(),
                method: req.method().clone(),
                path: req.url().path().to_string(),
                query: req.url().query().map(|q| q.to_string()),
            })
    } else {
        None
    };
    // 捕捉に失敗したとき(コールバック設定時のみ意味を持つ)は、ログ上 method/path/query/headers が
    // すべて None になる理由を debug ログに残しておく(ログは観測の副経路なので失敗しても続行)。
    if need_log && captured.is_none() {
        tracing::debug!(
            "request capture (try_clone/build) failed; headers/method/path/query will be None in logs"
        );
    }

    let redacted_body_keys = options.get_redacted_body_keys();

    if let Some(cb) = &options.on_request {
        run_log_callback("on_request", || {
            cb(&LineRequestLog::new(
                captured.as_ref(),
                request_value,
                redacted_body_keys,
            ));
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
                &LineRequestLog::new(captured.as_ref(), request_value, redacted_body_keys),
                &LineResponseLog::new(
                    &response_headers,
                    response_body,
                    status_code,
                    redacted_body_keys,
                ),
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
        // エラーステータスのボディも ErrorResponse → LineLoginErrorResponse → OtherJson の
        // 順で分類する(execute_api の成功経路と同じ順序に揃える)。
        // LINE Login 系の invalid_grant などは LineLoginErrorResponse 形式で返るため。
        match serde_json::from_value::<ErrorResponse>(json.clone()) {
            Ok(error_response) => Err(Box::new(Error::Line(
                error_response,
                status_code,
                line_header,
            ))),
            Err(_) => match serde_json::from_value::<LineLoginErrorResponse>(json.clone()) {
                Ok(error_response) => Err(Box::new(Error::LineLogin(
                    error_response,
                    status_code,
                    line_header,
                ))),
                Err(_) => Err(Box::new(Error::OtherJson(json, status_code, line_header))),
            },
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
                        Err(_) => {
                            match serde_json::from_value::<LineLoginErrorResponse>(json.clone()) {
                                Ok(error_response) => {
                                    Err(Error::LineLogin(error_response, status_code, line_header))
                                }
                                Err(_) => Err(Error::OtherJson(json, status_code, line_header)),
                            }
                        }
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
        let options = LineOptions::builder()
            .with_on_request(|_log| panic!("on_request callback panics"))
            .with_on_response(|_req, _res| panic!("on_response callback panics"))
            .build();

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

    // 補足: レスポンスボディの読取失敗(`response.text()` のエラー)が Error::Reqwest として
    // 伝播する経路(execute_api_raw)は、mockito では決定的に途中切断を起こしにくいため
    // ユニットテスト化していない。コード上は `.map_err(Error::Reqwest)?` で握り潰さず伝播する。

    // エラーステータス(400)で invalid_grant 形式のボディは Error::LineLogin に分類される。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_error_status_invalid_grant_is_line_login() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/test")
            .with_status(400)
            .with_body(r#"{"error":"invalid_grant","error_description":"invalid grant"}"#)
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default();

        let result: Result<(serde_json::Value, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().post(&url),
            &options,
            is_standard_retry,
            None,
            || serde_json::Value::Null,
        )
        .await;

        match result {
            Err(boxed) => match *boxed {
                Error::LineLogin(resp, status_code, _) => {
                    assert_eq!(resp.error, "invalid_grant");
                    assert_eq!(status_code, StatusCode::BAD_REQUEST);
                }
                other => panic!("expected Error::LineLogin, got {other:?}"),
            },
            Ok(_) => panic!("expected an error"),
        }
        mock.assert_async().await;
    }

    // 成功ステータス(200)でも、ボディが目的の型に合致せず LineLoginErrorResponse 形式の
    // 場合は Error::LineLogin に分類される(execute_api の成功経路側の分岐)。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_success_status_line_login_error_body() {
        #[derive(serde::Deserialize, Debug)]
        struct Dummy {
            #[allow(dead_code)]
            id: u64,
        }

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/test")
            .with_status(200)
            .with_body(r#"{"error":"invalid_grant","error_description":"invalid grant"}"#)
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default();

        let result: Result<(Dummy, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().post(&url),
            &options,
            is_standard_retry,
            None,
            || serde_json::Value::Null,
        )
        .await;

        match result {
            Err(boxed) => match *boxed {
                Error::LineLogin(resp, status_code, _) => {
                    assert_eq!(resp.error, "invalid_grant");
                    assert_eq!(status_code, StatusCode::OK);
                }
                other => panic!("expected Error::LineLogin, got {other:?}"),
            },
            Ok(_) => panic!("expected an error"),
        }
        mock.assert_async().await;
    }

    // エラーステータスで message を持つボディは LineLoginErrorResponse より先に
    // ErrorResponse として判定され、Error::Line に分類される(優先順位の確認)。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_error_status_message_is_line() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/test")
            .with_status(400)
            .with_body(r#"{"message":"bad request"}"#)
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default();

        let result: Result<(serde_json::Value, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().post(&url),
            &options,
            is_standard_retry,
            None,
            || serde_json::Value::Null,
        )
        .await;

        match result {
            Err(boxed) => match *boxed {
                Error::Line(resp, status_code, _) => {
                    assert_eq!(resp.message, "bad request");
                    assert_eq!(status_code, StatusCode::BAD_REQUEST);
                }
                other => panic!("expected Error::Line, got {other:?}"),
            },
            Ok(_) => panic!("expected an error"),
        }
        mock.assert_async().await;
    }

    // どちらの形式にも合致しないボディは Error::OtherJson にフォールバックする。
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn test_error_status_unrecognized_is_other_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/test")
            .with_status(400)
            .with_body(r#"{"foo":"bar"}"#)
            .create_async()
            .await;
        let url = format!("{}/test", server.url());
        let options = LineOptions::default();

        let result: Result<(serde_json::Value, LineResponseHeader), _> = execute_api(
            || reqwest::Client::new().post(&url),
            &options,
            is_standard_retry,
            None,
            || serde_json::Value::Null,
        )
        .await;

        match result {
            Err(boxed) => match *boxed {
                Error::OtherJson(json, status_code, _) => {
                    assert_eq!(json["foo"], "bar");
                    assert_eq!(status_code, StatusCode::BAD_REQUEST);
                }
                other => panic!("expected Error::OtherJson, got {other:?}"),
            },
            Ok(_) => panic!("expected an error"),
        }
        mock.assert_async().await;
    }
}
