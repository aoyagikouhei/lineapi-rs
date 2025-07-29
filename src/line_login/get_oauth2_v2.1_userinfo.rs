use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{apply_auth, apply_timeout, error::Error, is_standard_retry, make_url, line_login::execute_api, LineOptions, LineResponseHeader};

// https://developers.line.biz/ja/reference/line-login/#userinfo
const URL: &str = "/oauth2/v2.1/userinfo";

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    pub sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn build_get(access_token: &str, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url);
    request_builder = apply_auth(request_builder, access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub fn build_post(access_token: &str, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.post(&url);
    request_builder = apply_auth(request_builder, access_token);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute_get(
    access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(
        || build_get(access_token, options),
        options,
        is_standard_retry,
    )
    .await
}

pub async fn execute_post(
    access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(
        || build_post(access_token, options),
        options,
        is_standard_retry,
    )
    .await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::LineOptions;

    // ACCESS_TOKEN=xxx cargo test test_line_login_get_oauth2_v2_1_userinfo -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_get_oauth2_v2_1_userinfo() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        
        // Test GET method
        let (response, header) = super::execute_get(&access_token, &options)
            .await
            .unwrap();
        println!("GET Response: {}", serde_json::to_value(&response).unwrap());
        println!("GET Header: {:?}", header);
        
        // Test POST method
        let (response, header) = super::execute_post(&access_token, &options)
            .await
            .unwrap();
        println!("POST Response: {}", serde_json::to_value(&response).unwrap());
        println!("POST Header: {:?}", header);
    }
}