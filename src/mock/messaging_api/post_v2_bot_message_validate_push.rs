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
    pub messages: Vec<serde_json::Value>,
    pub status_code: usize,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.messages.is_none() {
        builder.messages(vec![json!({"type": "text", "text": "Hello, world!"})]);
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        // For validation endpoints, success typically returns empty body or just {}
        json!({})
    } else {
        json!({
            "message": params.error_message
        })
    };

    let expected_body = json!({
        "messages": params.messages
    });

    server
        .mock("POST", "/v2/bot/message/validate/push")
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
    use crate::{LineOptions, error::Error, messaging_api::post_v2_bot_message_validate_push};

    use super::*;

    // cargo test --all-features test_make_mock_post_v2_bot_message_validate_push_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_validate_push_success() {
        let mut server = Server::new_async().await;
        let messages = vec![
            json!({"type": "text", "text": "Hello World!"}),
            json!({"type": "text", "text": "How are you?"}),
        ];
        let mut builder = MockParamsBuilder::default();
        builder.messages(messages.clone());
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_v2_bot_message_validate_push::RequestBody { messages };

        let res = post_v2_bot_message_validate_push::execute(
            request_body,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // For validation endpoint, we expect an empty JSON object on success
        assert_eq!(res.0, json!({}));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_v2_bot_message_validate_push_with_image -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_validate_push_with_image() {
        let mut server = Server::new_async().await;
        let messages = vec![json!({
            "type": "image",
            "originalContentUrl": "https://example.com/original.jpg",
            "previewImageUrl": "https://example.com/preview.jpg"
        })];
        let mut builder = MockParamsBuilder::default();
        builder.messages(messages.clone());
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_v2_bot_message_validate_push::RequestBody { messages };

        let res = post_v2_bot_message_validate_push::execute(
            request_body,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0, json!({}));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_v2_bot_message_validate_push_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_v2_bot_message_validate_push_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        builder.error_message("Invalid message format".to_string());
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_v2_bot_message_validate_push::RequestBody {
            messages: vec![json!({"type": "text", "text": "Hello, world!"})],
        };

        let res = post_v2_bot_message_validate_push::execute(
            request_body,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await;

        match res {
            Err(Error::Line(response, status_code, _header)) => {
                assert_eq!(status_code, 400);
                assert_eq!(response.message, "Invalid message format");
            }
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}
