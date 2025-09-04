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
    pub user_access_token: String,
    pub status_code: usize,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.user_access_token.is_none() {
        builder.user_access_token("test_user_access_token".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 || params.status_code == 204 {
        json!({}).to_string()
    } else {
        json!({
            "message": params.error_message
        }).to_string()
    };

    let expected_body = json!({
        "userAccessToken": params.user_access_token.clone()
    });

    server
        .mock("POST", "/user/v1/deauthorize")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        )
        .match_body(mockito::Matcher::Json(expected_body))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json)
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::post_user_v1_deauthorize};

    use super::*;

    // cargo test --all-features test_make_mock_post_user_v1_deauthorize_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_user_v1_deauthorize_success() {
        let mut server = Server::new_async().await;
        let builder = MockParamsBuilder::default();
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_user_v1_deauthorize::RequestBody {
            user_access_token: "test_user_access_token".to_string(),
        };

        let res = post_user_v1_deauthorize::execute(
            "test_channel_access_token",
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Response should be empty for successful deauthorization
        assert!(res.0.extra.is_empty());

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_user_v1_deauthorize_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_user_v1_deauthorize_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_user_v1_deauthorize::RequestBody {
            user_access_token: "test_user_access_token".to_string(),
        };

        let res = post_user_v1_deauthorize::execute(
            "test_channel_access_token",
            &request_body,
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