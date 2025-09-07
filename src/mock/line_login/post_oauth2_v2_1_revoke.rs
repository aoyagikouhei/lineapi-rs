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
    pub client_id: String,
    pub client_secret: Option<String>,
    pub status_code: usize,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.access_token.is_none() {
        builder.access_token("test_access_token".to_string());
    }
    if builder.client_id.is_none() {
        builder.client_id("1234567890".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        json!({})
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("POST", "/oauth2/v2.1/revoke")
        .match_body(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("access_token".into(), params.access_token.clone()),
            mockito::Matcher::UrlEncoded("client_id".into(), params.client_id.clone()),
        ]))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::post_oauth2_v2_1_revoke};

    use super::*;

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_revoke_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_revoke_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.client_secret(Some("test_secret".to_string()));
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_revoke::RequestBody {
            access_token: "test_access_token".to_string(),
            client_id: "1234567890".to_string(),
            client_secret: Some("test_secret".to_string()),
        };

        let res = post_oauth2_v2_1_revoke::execute(
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Response should be empty for successful revocation
        assert!(res.0.extra.is_empty());

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_revoke_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_revoke_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_revoke::RequestBody {
            access_token: "test_access_token".to_string(),
            client_id: "1234567890".to_string(),
            client_secret: None,
        };

        let res = post_oauth2_v2_1_revoke::execute(
            &request_body,
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
