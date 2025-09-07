use derive_builder::Builder;
use mockito::{Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
    pub status_code: usize,
    pub access_token: String,
    pub expires_in: u64,
    pub id_token: Option<String>,
    pub response_refresh_token: String,
    pub scope: String,
    pub token_type: String,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.grant_type.is_none() {
        builder.grant_type("authorization_code".to_string());
    }
    if builder.code.is_none() && builder.grant_type.as_deref() == Some("authorization_code") {
        builder.code("test_code".to_string());
    }
    if builder.redirect_uri.is_none() && builder.grant_type.as_deref() == Some("authorization_code")
    {
        builder.redirect_uri("https://example.com/callback".to_string());
    }
    if builder.refresh_token.is_none() && builder.grant_type.as_deref() == Some("refresh_token") {
        builder.refresh_token("test_refresh_token".to_string());
    }
    if builder.client_id.is_none() {
        builder.client_id("1234567890".to_string());
    }
    if builder.client_secret.is_none() {
        builder.client_secret("test_secret".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.access_token.is_none() {
        builder.access_token("new_access_token".to_string());
    }
    if builder.expires_in.is_none() {
        builder.expires_in(2592000u64);
    }
    if builder.response_refresh_token.is_none() {
        builder.response_refresh_token("new_refresh_token".to_string());
    }
    if builder.scope.is_none() {
        builder.scope("profile openid".to_string());
    }
    if builder.token_type.is_none() {
        builder.token_type("Bearer".to_string());
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let mut json = json!({
            "access_token": params.access_token,
            "expires_in": params.expires_in,
            "refresh_token": params.response_refresh_token,
            "scope": params.scope,
            "token_type": params.token_type,
        });
        if params.id_token.is_some() {
            json["id_token"] = params.id_token.clone().into();
        }
        json
    } else {
        json!({
            "message": params.error_message
        })
    };

    let mut matchers = vec![
        mockito::Matcher::UrlEncoded("grant_type".into(), params.grant_type.clone()),
        mockito::Matcher::UrlEncoded("client_id".into(), params.client_id.clone()),
    ];

    if params.grant_type == "authorization_code" {
        if let Some(code) = params.code {
            matchers.push(mockito::Matcher::UrlEncoded("code".into(), code));
        }
        if let Some(redirect_uri) = params.redirect_uri {
            matchers.push(mockito::Matcher::UrlEncoded(
                "redirect_uri".into(),
                redirect_uri,
            ));
        }
        if let Some(client_secret) = params.client_secret.clone() {
            matchers.push(mockito::Matcher::UrlEncoded(
                "client_secret".into(),
                client_secret,
            ));
        }
    } else if params.grant_type == "refresh_token"
        && let Some(refresh_token) = params.refresh_token
    {
        matchers.push(mockito::Matcher::UrlEncoded(
            "refresh_token".into(),
            refresh_token,
        ));
    }

    server
        .mock("POST", "/oauth2/v2.1/token")
        .match_body(mockito::Matcher::AllOf(matchers))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::post_oauth2_v2_1_token};

    use super::*;

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_token_authorization_code -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_token_authorization_code() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.grant_type("authorization_code".to_string());
        builder.code("auth_code_123".to_string());
        builder.redirect_uri("https://myapp.com/callback".to_string());
        builder.id_token(Some("test_id_token".to_string()));
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_token::RequestBody::AuthorizationCode {
            code: "auth_code_123".to_string(),
            redirect_uri: "https://myapp.com/callback".to_string(),
            client_id: "1234567890".to_string(),
            client_secret: "test_secret".to_string(),
            code_verifier: None,
        };

        let res = post_oauth2_v2_1_token::execute(
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.access_token, "new_access_token");
        assert_eq!(res.0.expires_in, 2592000);
        assert_eq!(res.0.refresh_token, "new_refresh_token");
        assert_eq!(res.0.scope, "profile openid");
        assert_eq!(res.0.token_type, "Bearer");
        assert_eq!(res.0.id_token, Some("test_id_token".to_string()));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_token_refresh -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_token_refresh() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.grant_type("refresh_token".to_string());
        builder.refresh_token("old_refresh_token".to_string());
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_token::RequestBody::RefreshToken {
            refresh_token: "old_refresh_token".to_string(),
            client_id: "1234567890".to_string(),
            client_secret: Some("test_secret".to_string()),
        };

        let res = post_oauth2_v2_1_token::execute(
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.access_token, "new_access_token");
        assert_eq!(res.0.expires_in, 2592000);
        assert_eq!(res.0.refresh_token, "new_refresh_token");
        assert_eq!(res.0.scope, "profile openid");
        assert_eq!(res.0.token_type, "Bearer");

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_token_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_token_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_token::RequestBody::AuthorizationCode {
            code: "test_code".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            client_id: "1234567890".to_string(),
            client_secret: "test_secret".to_string(),
            code_verifier: None,
        };

        let res = post_oauth2_v2_1_token::execute(
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
