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
    pub friend_flag: bool,
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
    if builder.friend_flag.is_none() {
        builder.friend_flag(false);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        json!({
            "friendFlag": params.friend_flag,
        })
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/friendship/v1/status")
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
    use crate::{LineOptions, error::Error, line_login::get_friendship_v1_status};

    use super::*;

    // cargo test --all-features test_make_mock_get_friendship_v1_status_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_friendship_v1_status_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.friend_flag(true);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_friendship_v1_status::execute(
            "test_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.friend_flag, true);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_friendship_v1_status_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_friendship_v1_status_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_friendship_v1_status::execute(
            "test_access_token",
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
                },
                _ => panic!("Unexpected error"),
            }
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}
