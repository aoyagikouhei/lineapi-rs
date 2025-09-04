use derive_builder::Builder;
use mockito::{Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub id_token: String,
    pub client_id: String,
    pub nonce: Option<String>,
    pub user_id: Option<String>,
    pub status_code: usize,
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    pub auth_time: Option<u64>,
    pub response_nonce: Option<String>,
    pub amr: Vec<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub email: Option<String>,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.id_token.is_none() {
        builder.id_token("test_id_token".to_string());
    }
    if builder.client_id.is_none() {
        builder.client_id("1234567890".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.iss.is_none() {
        builder.iss("https://access.line.me".to_string());
    }
    if builder.sub.is_none() {
        builder.sub("U123456".to_string());
    }
    if builder.aud.is_none() {
        builder.aud("1234567890".to_string());
    }
    if builder.exp.is_none() {
        builder.exp(1700000000u64);
    }
    if builder.iat.is_none() {
        builder.iat(1699996400u64);
    }
    if builder.amr.is_none() {
        builder.amr(vec!["linesso".to_string()]);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let mut json = json!({
            "iss": params.iss,
            "sub": params.sub,
            "aud": params.aud,
            "exp": params.exp,
            "iat": params.iat,
            "amr": params.amr,
        });
        if params.auth_time.is_some() {
            json["auth_time"] = params.auth_time.into();
        }
        if params.response_nonce.is_some() {
            json["nonce"] = params.response_nonce.clone().into();
        }
        if params.name.is_some() {
            json["name"] = params.name.clone().into();
        }
        if params.picture.is_some() {
            json["picture"] = params.picture.clone().into();
        }
        if params.email.is_some() {
            json["email"] = params.email.clone().into();
        }
        json
    } else {
        json!({
            "message": params.error_message
        })
    };

    let mut matchers = vec![
        mockito::Matcher::UrlEncoded("id_token".into(), params.id_token.clone()),
        mockito::Matcher::UrlEncoded("client_id".into(), params.client_id.clone()),
    ];

    if let Some(nonce) = params.nonce {
        matchers.push(mockito::Matcher::UrlEncoded("nonce".into(), nonce));
    }
    if let Some(user_id) = params.user_id {
        matchers.push(mockito::Matcher::UrlEncoded("user_id".into(), user_id));
    }

    server
        .mock("POST", "/oauth2/v2.1/verify")
        .match_body(mockito::Matcher::AllOf(matchers))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, line_login::post_oauth2_v2_1_verify};

    use super::*;

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_verify_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_verify_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.response_nonce(Some("test_nonce".to_string()));
        builder.auth_time(Some(1699996000u64));
        builder.name(Some("Test User".to_string()));
        builder.picture(Some("https://example.com/picture.jpg".to_string()));
        builder.email(Some("test@example.com".to_string()));
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_verify::RequestBody {
            id_token: "test_id_token".to_string(),
            client_id: "1234567890".to_string(),
            nonce: Some("test_nonce".to_string()),
            user_id: None,
        };

        let res = post_oauth2_v2_1_verify::execute(
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.iss, "https://access.line.me");
        assert_eq!(res.0.sub, "U123456");
        assert_eq!(res.0.aud, "1234567890");
        assert_eq!(res.0.exp, 1700000000);
        assert_eq!(res.0.iat, 1699996400);
        assert_eq!(res.0.auth_time, Some(1699996000));
        assert_eq!(res.0.nonce, Some("test_nonce".to_string()));
        assert_eq!(res.0.amr, vec!["linesso"]);
        assert_eq!(res.0.name, Some("Test User".to_string()));
        assert_eq!(res.0.picture, Some("https://example.com/picture.jpg".to_string()));
        assert_eq!(res.0.email, Some("test@example.com".to_string()));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_verify_minimal -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_verify_minimal() {
        let mut server = Server::new_async().await;
        let builder = MockParamsBuilder::default();
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_verify::RequestBody {
            id_token: "test_id_token".to_string(),
            client_id: "1234567890".to_string(),
            nonce: None,
            user_id: None,
        };

        let res = post_oauth2_v2_1_verify::execute(
            &request_body,
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.iss, "https://access.line.me");
        assert_eq!(res.0.sub, "U123456");
        assert_eq!(res.0.aud, "1234567890");
        assert!(res.0.nonce.is_none());
        assert!(res.0.name.is_none());
        assert!(res.0.picture.is_none());
        assert!(res.0.email.is_none());

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_post_oauth2_v2_1_verify_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_post_oauth2_v2_1_verify_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let request_body = post_oauth2_v2_1_verify::RequestBody {
            id_token: "test_id_token".to_string(),
            client_id: "1234567890".to_string(),
            nonce: None,
            user_id: None,
        };

        let res = post_oauth2_v2_1_verify::execute(
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