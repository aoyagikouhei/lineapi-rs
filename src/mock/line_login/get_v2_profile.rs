use derive_builder::Builder;
use mockito::{Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub access_token: String,
    pub status_code: usize,
    pub user_id: String,
    pub display_name: String,
    pub picture_url: Option<String>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.access_token.is_none() {
        builder.access_token("test_access_token".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.user_id.is_none() {
        builder.user_id("U123456".to_string());
    }
    if builder.display_name.is_none() {
        builder.display_name("Test User".to_string());
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let mut json = json!({
            "userId": params.user_id,
            "displayName": params.display_name,
        });
        if params.picture_url.is_some() {
            json["pictureUrl"] = params.picture_url.clone().into();
        }
        if params.status_message.is_some() {
            json["statusMessage"] = params.status_message.clone().into();
        }
        json
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/v2/profile")
        .match_header(
            "authorization",
            format!("Bearer {}", params.access_token).as_str(),
        )
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::get_v2_profile};

    use super::*;

    // cargo test --all-features test_make_mock_get_v2_profile_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_profile_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.picture_url(Some("https://example.com/picture.jpg".into()));
        builder.status_message(Some("I'm happy!".into()));
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_profile::execute(
            "test_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.user_id, "U123456");
        assert_eq!(res.0.display_name, "Test User");
        assert_eq!(
            res.0.picture_url,
            Some("https://example.com/picture.jpg".into())
        );
        assert_eq!(res.0.status_message, Some("I'm happy!".into()));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_profile_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_profile_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_profile::execute(
            "test_access_token",
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
