use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    LineOptions, LineResponseHeader, apply_timeout, error::Error, execute_api, is_standard_retry,
    make_url,
};

// https://developers.line.biz/ja/reference/line-login/#verify-access-token
const URL: &str = "/oauth2/v2.1/verify";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseBody {
    pub scope: String,
    pub client_id: String,
    pub expires_in: u64,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn build(access_token: &str, options: &LineOptions) -> RequestBuilder {
    let url = make_url(URL, options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url);
    request_builder = request_builder.query(&[("access_token", access_token)]);
    request_builder = apply_timeout(request_builder, options);
    request_builder
}

pub async fn execute(
    access_token: &str,
    options: &LineOptions,
) -> Result<(ResponseBody, LineResponseHeader), Box<Error>> {
    execute_api(
        || build(access_token, options),
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

    use crate::LineOptions;

    // ACCESS_TOKEN=xxx cargo test test_line_login_get_oauth2_v2_1_verify -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_line_login_get_oauth2_v2_1_verify() {
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

        let (response, header) = super::execute(&access_token, &options).await.unwrap();
        println!("{}", serde_json::to_value(&response).unwrap());
        println!("{header:?}");
    }
}
