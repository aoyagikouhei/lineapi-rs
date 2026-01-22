use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    LineOptions, LineResponseHeader, apply_timeout, error::Error, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/line-login/#issue-access-token
const URL: &str = "/oauth2/v2.1/token";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "grant_type")]
pub enum RequestBody {
    #[serde(rename = "authorization_code")]
    AuthorizationCode {
        code: String,
        redirect_uri: String,
        client_id: String,
        client_secret: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code_verifier: Option<String>,
    },
    #[serde(rename = "refresh_token")]
    RefreshToken {
        refresh_token: String,
        client_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseBody {
    pub access_token: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn build(request_body: &RequestBody, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.post(&url);
    request_builder = request_builder.form(request_body);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    request_body: &RequestBody,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Box<Error>> {
    execute_api(
        || build(request_body, options),
        options,
        is_standard_retry,
        None,
    )
    .await
}

pub async fn execute_authorization_code(
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: &str,
    code_verifier: Option<String>,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Box<Error>> {
    let request_body = RequestBody::AuthorizationCode {
        code: code.to_string(),
        redirect_uri: redirect_uri.to_string(),
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        code_verifier,
    };
    execute(&request_body, options).await
}

pub async fn execute_refresh_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<String>,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Box<Error>> {
    let request_body = RequestBody::RefreshToken {
        refresh_token: refresh_token.to_string(),
        client_id: client_id.to_string(),
        client_secret,
    };
    execute(&request_body, options).await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::LineOptions;

    // CODE=xxx REDIRECT_URI=xxx CLIENT_ID=xxx CLIENT_SECRET=xxx cargo test test_line_login_post_oauth2_v2_1_token_authorization_code -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_post_oauth2_v2_1_token_authorization_code() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let code = std::env::var("CODE").unwrap();
        let redirect_uri = std::env::var("REDIRECT_URI").unwrap();
        let client_id = std::env::var("CLIENT_ID").unwrap();
        let client_secret = std::env::var("CLIENT_SECRET").unwrap();

        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };

        let (response, header) = super::execute_authorization_code(
            &code,
            &redirect_uri,
            &client_id,
            &client_secret,
            None,
            &options,
        )
        .await
        .unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{header:?}");
    }

    // REFRESH_TOKEN=xxx CLIENT_ID=xxx CLIENT_SECRET=xxx cargo test test_line_login_post_oauth2_v2_1_token_refresh_token -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_post_oauth2_v2_1_token_refresh_token() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let refresh_token = std::env::var("REFRESH_TOKEN").unwrap();
        let client_id = std::env::var("CLIENT_ID").unwrap();
        let client_secret = std::env::var("CLIENT_SECRET").ok();

        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };

        let (response, header) =
            super::execute_refresh_token(&refresh_token, &client_id, client_secret, &options)
                .await
                .unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{header:?}");
    }
}
