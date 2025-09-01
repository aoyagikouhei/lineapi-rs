use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    LineOptions, LineResponseHeader, apply_timeout, error::Error, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/line-login/#verify-id-token
const URL: &str = "/oauth2/v2.1/verify";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestBody {
    pub id_token: String,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseBody {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    pub amr: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
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
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(
        || build(request_body, options),
        options,
        is_standard_retry,
        false,
    )
    .await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::{LineOptions, line_login::post_oauth2_v2_1_verify};

    // ID_TOKEN=xxx CLIENT_ID=xxx cargo test test_line_login_post_oauth2_v2_1_verify -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_post_oauth2_v2_1_verify() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let id_token = std::env::var("ID_TOKEN").unwrap();
        let client_id = std::env::var("CLIENT_ID").unwrap();

        let request_body = post_oauth2_v2_1_verify::RequestBody {
            id_token,
            client_id,
            nonce: None,
            user_id: None,
        };

        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };

        let (response, header) = post_oauth2_v2_1_verify::execute(&request_body, &options)
            .await
            .unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{header:?}");
    }
}
