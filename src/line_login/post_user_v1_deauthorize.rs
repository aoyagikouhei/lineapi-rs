use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{apply_auth, apply_timeout, error::Error, is_standard_retry, make_url, line_login::execute_api, LineOptions, LineResponseHeader};

// https://developers.line.biz/ja/reference/line-login/#deauthorize-app
const URL: &str = "/user/v1/deauthorize";

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    #[serde(rename = "userAccessToken")]
    pub user_access_token: String,
}

// The response is empty for this endpoint (204 status)
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn build(channel_access_token: &str, request_body: &RequestBody, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.post(&url);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = request_builder.json(request_body);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    channel_access_token: &str,
    request_body: &RequestBody,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(
        || build(channel_access_token, request_body, options),
        options,
        is_standard_retry,
    )
    .await
}

pub async fn execute_simple(
    channel_access_token: &str,
    user_access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Error> {
    let request_body = RequestBody {
        user_access_token: user_access_token.to_string(),
    };
    execute(channel_access_token, &request_body, options).await
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::LineOptions;

    // CHANNEL_ACCESS_TOKEN=xxx USER_ACCESS_TOKEN=xxx cargo test test_line_login_post_user_v1_deauthorize -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_post_user_v1_deauthorize() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let channel_access_token = std::env::var("CHANNEL_ACCESS_TOKEN").unwrap();
        let user_access_token = std::env::var("USER_ACCESS_TOKEN").unwrap();
        
        let options = LineOptions {
            try_count: Some(3),
            retry_duration: Some(std::time::Duration::from_secs(1)),
            ..Default::default()
        };
        
        let (response, header) = super::execute_simple(&channel_access_token, &user_access_token, &options)
            .await
            .unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{:?}", header);
    }
}