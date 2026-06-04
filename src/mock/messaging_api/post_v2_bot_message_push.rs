use derive_builder::Builder;
use mockito::{Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub channel_access_token: String,
    pub to: String,
    pub messages: Vec<serde_json::Value>,
    pub notification_disabled: Option<bool>,
    pub custom_aggregation_units: Option<Vec<String>>,
    pub status_code: usize,
    pub sent_message_ids: Vec<String>,
    pub sent_message_quote_tokens: Vec<Option<String>>,
    pub response_message: Option<String>,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.to.is_none() {
        builder.to("U123456789".to_string());
    }
    if builder.messages.is_none() {
        builder.messages(vec![json!({"type": "text", "text": "Hello!"})]);
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.sent_message_ids.is_none() {
        builder.sent_message_ids(vec!["msg123".to_string()]);
    }
    if builder.sent_message_quote_tokens.is_none() {
        builder.sent_message_quote_tokens(vec![Some("token123".to_string())]);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let sent_messages: Vec<serde_json::Value> = params
            .sent_message_ids
            .iter()
            .zip(params.sent_message_quote_tokens.iter())
            .map(|(id, quote_token)| {
                let mut msg = json!({
                    "id": id
                });
                if let Some(token) = quote_token {
                    msg["quoteToken"] = json!(token);
                }
                msg
            })
            .collect();

        let mut response = json!({
            "sentMessages": sent_messages
        });
        if let Some(msg) = params.response_message {
            response["message"] = json!(msg);
        }
        response
    } else {
        json!({
            "message": params.error_message
        })
    };

    let expected_body =
        if params.notification_disabled.is_some() || params.custom_aggregation_units.is_some() {
            let mut body = json!({
                "to": params.to,
                "messages": params.messages
            });
            if let Some(notification_disabled) = params.notification_disabled {
                body["notificationDisabled"] = json!(notification_disabled);
            }
            if let Some(custom_aggregation_units) = params.custom_aggregation_units {
                body["customAggregationUnits"] = json!(custom_aggregation_units);
            }
            body
        } else {
            json!({
                "to": params.to,
                "messages": params.messages
            })
        };

    server
        .mock("POST", "/v2/bot/message/push")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        )
        .match_body(mockito::Matcher::Json(expected_body))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{error::Error, messaging_api::post_v2_bot_message_push, option::LineOptions};

    use super::*;

    // cargo test --all-features test_make_mock_post_v2_bot_message_push_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_success() {
        let mut server = Server::new_async().await;
        let messages = vec![json!({"type": "text", "text": "Hello World!"})];
        let mut builder = MockParamsBuilder::default();
        builder.messages(messages.clone());
        builder.sent_message_ids(vec!["msg456".to_string()]);
        builder.sent_message_quote_tokens(vec![Some("quote456".to_string())]);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body =
            post_v2_bot_message_push::RequestBody::new("U123456789", messages).unwrap();

        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &LineOptions::builder().with_prefix_url(server.url()).build(),
            None,
        )
        .await
        .unwrap();

        // Note: ResponseBody fields are private, so we can only verify successful execution
        // In a real implementation, you might want to make them public for testing

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_v2_bot_message_push_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();

        let res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &LineOptions::builder().with_prefix_url(server.url()).build(),
            None,
        )
        .await;

        match res {
            Err(e) => match *e {
                Error::Line(response, status_code, _header) => {
                    assert_eq!(status_code, 400);
                    assert_eq!(response.message, "error occurred");
                }
                _ => panic!("Unexpected error"),
            },
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        let messages = vec![json!({"type": "text", "text": "Hello World!"})];
        let mut builder = MockParamsBuilder::default();
        builder.messages(messages.clone());
        let mock = make_mock(&mut server, Some(builder)).await;

        // (has_authorization_header, request_body)
        let captured_req = Arc::new(Mutex::new(Vec::<(bool, serde_json::Value)>::new()));
        // (status_code, response_body)
        let captured_res = Arc::new(Mutex::new(Vec::<(u16, serde_json::Value)>::new()));

        let creq = captured_req.clone();
        let cres = captured_res.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_request(move |log| {
                let has_auth = log
                    .headers()
                    .is_some_and(|h| h.contains_key("authorization"));
                creq.lock().unwrap().push((has_auth, log.body().clone()));
            })
            .with_on_response(move |_req, res| {
                cres.lock()
                    .unwrap()
                    .push((res.status_code().as_u16(), res.as_value().into_owned()));
            })
            .build();

        let request_body =
            post_v2_bot_message_push::RequestBody::new("U123456789", messages).unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await
        .unwrap();

        mock.assert_async().await;

        // on_request: 1回、Authorizationヘッダー付き、bodyにto/messagesを含む
        let reqs = captured_req.lock().unwrap();
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].0, "authorization header must be present");
        assert_eq!(reqs[0].1["to"], json!("U123456789"));
        assert!(reqs[0].1["messages"].is_array());

        // on_response: 1回、status 200、レスポンスJSONを含む
        let ress = captured_res.lock().unwrap();
        assert_eq!(ress.len(), 1);
        assert_eq!(ress[0].0, 200);
        assert!(ress[0].1["sentMessages"].is_array());
    }

    // on_response 設定時に response.headers() を複製してコールバックへ渡す分岐を保護する。
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks_response_headers -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks_response_headers() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        let mock = make_mock(&mut server, None).await;

        // レスポンスヘッダーに content-type が含まれるか
        let has_content_type = Arc::new(Mutex::new(false));
        let hct = has_content_type.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_response(move |_req, res| {
                *hct.lock().unwrap() = res.headers().contains_key("content-type");
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await
        .unwrap();

        mock.assert_async().await;
        assert!(
            *has_content_type.lock().unwrap(),
            "on_response must receive the cloned response headers (content-type)"
        );
    }

    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks_retry -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks_retry() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(500usize);
        let mock = make_mock(&mut server, Some(builder)).await.expect(3);

        let req_count = Arc::new(Mutex::new(0usize));
        let res_count = Arc::new(Mutex::new(0usize));
        let rc = req_count.clone();
        let sc = res_count.clone();

        // try_count=3, retry_duration=0 (待機なし) で試行ごとに発火することを確認
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_try_count(3)
            .with_on_request(move |_log| {
                *rc.lock().unwrap() += 1;
            })
            .with_on_response(move |_req, _res| {
                *sc.lock().unwrap() += 1;
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await;

        assert_eq!(
            *req_count.lock().unwrap(),
            3,
            "on_request fires per attempt"
        );
        assert_eq!(
            *res_count.lock().unwrap(),
            3,
            "on_response fires per attempt"
        );

        mock.assert_async().await;
    }

    // 非JSONレスポンス: on_response の body() が生テキストを Value::String で受け取る
    // 409 CONFLICT: retry_key 付き push は成功扱いだが、コールバックは 409 を観測する
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks_conflict -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks_conflict() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        // 409 でも「配信済み」扱いとするため、ボディは正常な sentMessages 形式で返す
        let mock = server
            .mock("POST", "/v2/bot/message/push")
            .with_status(409)
            .with_header("content-type", "application/json")
            .with_body(json!({"sentMessages": [{"id": "msg123"}]}).to_string())
            .create_async()
            .await;

        let captured = Arc::new(Mutex::new(Vec::<u16>::new()));
        let c = captured.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_try_count(2)
            .with_on_response(move |_req, res| {
                c.lock().unwrap().push(res.status_code().as_u16());
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            Some("retry-key-1".to_string()),
        )
        .await;

        mock.assert_async().await;

        // コールバックは 409 を観測
        assert_eq!(*captured.lock().unwrap(), vec![409]);
        // retry_key 付き 409 は配信済みとして Ok 扱い
        assert!(res.is_ok(), "409 with retry_key is treated as delivered");
    }

    // リトライキーのヘッダー捕捉: clone が retry-key 付与「後」であることを保護
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks_retry_key_header -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks_retry_key_header() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        // 200 を返すので 1 回で成功するが、try_count=2 なので retry-key ヘッダーは付く
        let mock = make_mock(&mut server, Some(MockParamsBuilder::default())).await;

        let captured = Arc::new(Mutex::new(Vec::<bool>::new()));
        let c = captured.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_try_count(2)
            .with_on_request(move |log| {
                let has_retry_key = log
                    .headers()
                    .is_some_and(|h| h.contains_key("x-line-retry-key"));
                c.lock().unwrap().push(has_retry_key);
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            Some("retry-key-1".to_string()),
        )
        .await
        .unwrap();

        mock.assert_async().await;

        // 捕捉したリクエストヘッダーに X-Line-Retry-Key が含まれる
        assert_eq!(*captured.lock().unwrap(), vec![true]);
    }

    // on_response のみ設定: need_log は両者の OR なのでリクエストヘッダーは捕捉される
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_callbacks_response_only -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_callbacks_response_only() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        let mock = make_mock(&mut server, Some(MockParamsBuilder::default())).await;

        let captured = Arc::new(Mutex::new(Vec::<bool>::new()));
        let c = captured.clone();
        // on_request は設定せず on_response のみ
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_response(move |req, _res| {
                let has_auth = req
                    .headers()
                    .is_some_and(|h| h.contains_key("authorization"));
                c.lock().unwrap().push(has_auth);
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await
        .unwrap();

        mock.assert_async().await;

        // on_request 未設定でもリクエストヘッダーは捕捉される
        assert_eq!(*captured.lock().unwrap(), vec![true]);
    }

    // headers_redacted: Authorization は *** にマスクされ、非秘匿ヘッダーは保持される
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_headers_redacted -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_headers_redacted() {
        use std::sync::{Arc, Mutex};

        let mut server = Server::new_async().await;
        let mock = make_mock(&mut server, Some(MockParamsBuilder::default())).await;

        // (authorization 値, content-type が存在するか)
        let captured = Arc::new(Mutex::new(Vec::<(Option<String>, bool)>::new()));
        let c = captured.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_request(move |log| {
                let redacted = log.headers_redacted().expect("headers captured");
                let auth = redacted
                    .get("authorization")
                    .map(|v| v.to_str().unwrap().to_string());
                let has_content_type = redacted.contains_key("content-type");
                c.lock().unwrap().push((auth, has_content_type));
            })
            .build();

        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await
        .unwrap();

        mock.assert_async().await;

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        // Authorization はマスクされ、元のトークンは含まない
        assert_eq!(captured[0].0, Some("***".to_string()));
        // content-type(非秘匿)は保持される
        assert!(captured[0].1, "non-secret header must be preserved");
    }

    // body_was_json: JSON レスポンスは true、非JSON は false
    // cargo test --all-features test_make_mock_post_v2_bot_message_push_body_was_json -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_push_body_was_json() {
        use std::sync::{Arc, Mutex};

        // --- JSON レスポンス: true ---
        let mut server = Server::new_async().await;
        let mock = make_mock(&mut server, Some(MockParamsBuilder::default())).await;
        let captured = Arc::new(Mutex::new(Vec::<bool>::new()));
        let c = captured.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_response(move |_req, res| {
                c.lock().unwrap().push(res.body_was_json());
            })
            .build();
        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let _ = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert_eq!(*captured.lock().unwrap(), vec![true]);

        // --- 非JSON レスポンス: false ---
        // make_mock は常にJSONを返すため、非JSONボディはここで直接組む。
        // body_was_json()==false / as_value()==生テキスト / execute は OtherText を返す、を一括検証。
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v2/bot/message/push")
            .with_status(502)
            .with_header("content-type", "text/plain")
            .with_body("Bad Gateway")
            .create_async()
            .await;
        // (body_was_json, as_value)
        let captured = Arc::new(Mutex::new(Vec::<(bool, serde_json::Value)>::new()));
        let c = captured.clone();
        let options = LineOptions::builder()
            .with_prefix_url(server.url())
            .with_on_response(move |_req, res| {
                c.lock()
                    .unwrap()
                    .push((res.body_was_json(), res.as_value().into_owned()));
            })
            .build();
        let request_body = post_v2_bot_message_push::RequestBody::new(
            "U123456789",
            vec![json!({"type": "text", "text": "Hello!"})],
        )
        .unwrap();
        let res = post_v2_bot_message_push::execute(
            request_body,
            "test_channel_access_token",
            &options,
            None,
        )
        .await;
        mock.assert_async().await;

        // コールバックは生テキストを Value::String で観測し body_was_json は false
        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(!captured[0].0, "非JSONなので body_was_json は false");
        assert_eq!(captured[0].1, json!("Bad Gateway"));
        // 非JSONなので execute は OtherText を返す
        assert!(matches!(*res.unwrap_err(), Error::OtherText(_, _, _)));
    }
}
