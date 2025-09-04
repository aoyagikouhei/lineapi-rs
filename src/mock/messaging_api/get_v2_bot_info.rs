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
    pub status_code: usize,
    pub user_id: String,
    pub basic_id: String,
    pub premium_id: Option<String>,
    pub display_name: String,
    pub picture_url: Option<String>,
    pub chat_mode: String,
    pub mark_as_read_mode: String,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.user_id.is_none() {
        builder.user_id("U123456789".to_string());
    }
    if builder.basic_id.is_none() {
        builder.basic_id("@testbot".to_string());
    }
    if builder.display_name.is_none() {
        builder.display_name("Test Bot".to_string());
    }
    if builder.chat_mode.is_none() {
        builder.chat_mode("bot".to_string());
    }
    if builder.mark_as_read_mode.is_none() {
        builder.mark_as_read_mode("auto".to_string());
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let mut json = json!({
            "userId": params.user_id,
            "basicId": params.basic_id,
            "displayName": params.display_name,
            "chatMode": params.chat_mode,
            "markAsReadMode": params.mark_as_read_mode,
        });
        if params.premium_id.is_some() {
            json["premiumId"] = params.premium_id.clone().into();
        }
        if params.picture_url.is_some() {
            json["pictureUrl"] = params.picture_url.clone().into();
        }
        json
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/v2/bot/info")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        )
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, messaging_api::get_v2_bot_info};

    use super::*;

    // cargo test --all-features test_make_mock_get_v2_bot_info_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_info_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.premium_id(Some("premium123".into()));
        builder.picture_url(Some("https://example.com/bot.jpg".into()));
        builder.chat_mode("chat".to_string());
        builder.mark_as_read_mode("manual".to_string());
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_bot_info::execute(
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.user_id, "U123456789");
        assert_eq!(res.0.basic_id, "@testbot");
        assert_eq!(res.0.premium_id, Some("premium123".into()));
        assert_eq!(res.0.display_name, "Test Bot");
        assert_eq!(res.0.picture_url, Some("https://example.com/bot.jpg".into()));
        assert_eq!(res.0.chat_mode, get_v2_bot_info::ChatMode::Chat);
        assert_eq!(res.0.mark_as_read_mode, get_v2_bot_info::MarkAsReadMode::Manual);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_info_minimal -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_info_minimal() {
        let mut server = Server::new_async().await;
        let builder = MockParamsBuilder::default();
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_bot_info::execute(
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.user_id, "U123456789");
        assert_eq!(res.0.basic_id, "@testbot");
        assert!(res.0.premium_id.is_none());
        assert_eq!(res.0.display_name, "Test Bot");
        assert!(res.0.picture_url.is_none());
        assert_eq!(res.0.chat_mode, get_v2_bot_info::ChatMode::Bot);
        assert_eq!(res.0.mark_as_read_mode, get_v2_bot_info::MarkAsReadMode::Auto);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_info_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_info_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_bot_info::execute(
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
                assert_eq!(response.message, "error occurred");
            }
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}