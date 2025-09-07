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
    pub scope: String,
    pub client_id: String,
    pub expires_in: u64,
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
    if builder.scope.is_none() {
        builder.scope("profile openid".to_string());
    }
    if builder.client_id.is_none() {
        builder.client_id("1234567890".to_string());
    }
    if builder.expires_in.is_none() {
        builder.expires_in(2591999u64);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        json!({
            "scope": params.scope,
            "client_id": params.client_id,
            "expires_in": params.expires_in,
        })
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/oauth2/v2.1/verify")
        .match_query(mockito::Matcher::UrlEncoded(
            "access_token".into(),
            params.access_token.clone(),
        ))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::get_oauth2_v2_1_verify};

    use super::*;

    // cargo test --all-features test_make_mock_get_oauth2_v2_1_verify_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_oauth2_v2_1_verify_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.scope("profile openid email".to_string());
        builder.expires_in(3600u64);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_oauth2_v2_1_verify::execute(
            "test_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.scope, "profile openid email");
        assert_eq!(res.0.client_id, "1234567890");
        assert_eq!(res.0.expires_in, 3600);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_oauth2_v2_1_verify_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_oauth2_v2_1_verify_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_oauth2_v2_1_verify::execute(
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
                }
                _ => panic!("Unexpected error"),
            },
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}
