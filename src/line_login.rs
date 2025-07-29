use std::time::Duration;

use rand::prelude::*;
use reqwest::{
    RequestBuilder, StatusCode
};
use serde::de::DeserializeOwned;

use crate::{calc_retry_duration, error::{Error, ErrorResponse}, execute_api_raw, LineOptions, LineResponseHeader};

pub mod get_v2_profile;
pub mod get_oauth2_v2_1_userinfo;
pub mod post_oauth2_v2_1_verify;
pub mod post_oauth2_v2_1_token;
pub mod get_oauth2_v2_1_verify;
pub mod post_oauth2_v2_1_revoke;
pub mod post_user_v1_deauthorize;
pub mod get_friendship_v1_status;

pub async fn execute_api<T, F>(
    f: impl Fn() -> RequestBuilder,
    options: &LineOptions,
    is_retry: F,
) -> Result<(T, LineResponseHeader), crate::error::Error>
where
    T: DeserializeOwned,
    F: Fn(StatusCode) -> bool,
{
    // リトライ処理
    let mut res = Err(Error::Invalid("fail loop".to_string()));
    let try_count = options.get_try_count();
    let retry_duration: Duration = options.get_retry_duration();
    let mut rng = StdRng::from_os_rng();
    for i in 0..try_count {
        // リクエスト準備
        let builder = f();
        match execute_api_raw(builder, false).await {
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
                    res = Err(err);
                    break;
                }

                if i + 1 >= try_count {
                    // リトライ回数がオーバーしたので失敗にする
                    res = Err(err);
                } else if !retry_duration.is_zero() {
                    // リトライ間隔がある場合は待つ
                    tokio::time::sleep(calc_retry_duration(retry_duration, i as u32, &mut rng))
                        .await;
                }
            }
        }
    }
    res
}
