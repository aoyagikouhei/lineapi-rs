use derive_builder::Builder;
use mockito::{Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub channel_access_token: String,
    pub status_code: usize,
    pub num_of_custom_aggregation_units: u64,
    pub error_message: Option<String>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.num_of_custom_aggregation_units.is_none() {
        builder.num_of_custom_aggregation_units(25u64);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        json!({
            "numOfCustomAggregationUnits": params.num_of_custom_aggregation_units,
        })
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/v2/bot/message/aggregation/info")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        )
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, messaging_api::get_v2_bot_message_aggregation_info};

    use super::*;

    // cargo test --all-features test_make_mock_get_v2_bot_message_aggregation_info_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_message_aggregation_info_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.num_of_custom_aggregation_units(42u64);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_bot_message_aggregation_info::execute(
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.num_of_custom_aggregation_units, 42);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_message_aggregation_info_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_message_aggregation_info_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(500usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let res = get_v2_bot_message_aggregation_info::execute(
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await;

        match res {
            Err(Error::Line(response, status_code, _header)) => {
                assert_eq!(status_code, 500);
                assert_eq!(response.message, "error occurred");
            }
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}
