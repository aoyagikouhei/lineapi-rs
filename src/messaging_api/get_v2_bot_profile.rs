use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::{apply_auth, apply_timeout, execute_api, make_url, LineOptions, LineResponseHeader};

// https://developers.line.biz/ja/reference/messaging-api/#get-profile
const URL: &str = "/v2/bot/profile";


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody {
    pub display_name: String,
    pub user_id: String,
    pub language: Option<String>,
    pub picture_url: Option<String>,
    pub status_message: Option<String>,
}

pub fn build(user_id: &str, channel_access_token: &str, options: &Option<LineOptions>) -> RequestBuilder {
    let url = make_url(&format!("{}/{}", URL, user_id), &options);
    let client = reqwest::Client::new();
    let mut request_builder = client.get(&url);
    request_builder = apply_auth(request_builder, channel_access_token);
    request_builder = apply_timeout(request_builder, &options);
    request_builder
}

pub async fn execute(user_id: &str, channel_access_token: &str, options: &Option<LineOptions>) -> Result<(ResponseBody, LineResponseHeader), Error> {
    execute_api(|| build(user_id, channel_access_token, options), options).await
}