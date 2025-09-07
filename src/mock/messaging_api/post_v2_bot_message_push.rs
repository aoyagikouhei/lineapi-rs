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
    use crate::{LineOptions, error::Error, messaging_api::post_v2_bot_message_push};

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
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
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
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
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
}
