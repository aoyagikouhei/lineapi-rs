use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    LineOptions, LineResponseHeader, apply_timeout, error::Error, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/line-login/#revoke-access-token
const URL: &str = "/oauth2/v2.1/revoke";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestBody {
    pub access_token: String,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

// The response is empty for this endpoint
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseBody {
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
        false,
    )
    .await
}

pub async fn execute_simple(
    access_token: &str,
    client_id: &str,
    client_secret: Option<String>,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Box<Error>> {
    let request_body = RequestBody {
        access_token: access_token.to_string(),
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

    // ACCESS_TOKEN=xxx CLIENT_ID=xxx CLIENT_SECRET=xxx cargo test test_line_login_post_oauth2_v2_1_revoke -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_post_oauth2_v2_1_revoke() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let client_id = std::env::var("CLIENT_ID").unwrap();
        let client_secret = std::env::var("CLIENT_SECRET").ok();

        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };

        let (response, header) =
            super::execute_simple(&access_token, &client_id, client_secret, &options)
                .await
                .unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{header:?}");
    }
}
