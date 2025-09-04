use derive_builder::Builder;
use mockito::{Mock, Server, Matcher};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub channel_access_token: String,
    pub limit: Option<u8>,
    pub start: Option<String>,
    pub status_code: usize,
    pub custom_aggregation_units: Vec<String>,
    pub next: Option<String>,
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
    if builder.custom_aggregation_units.is_none() {
        builder.custom_aggregation_units(vec!["promotion_a".to_string(), "promotion_b".to_string()]);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let mut response = json!({
            "customAggregationUnits": params.custom_aggregation_units,
        });
        if let Some(next) = params.next {
            response["next"] = json!(next);
        }
        response
    } else {
        json!({
            "message": params.error_message
        })
    };

    let mut mock_builder = server
        .mock("GET", "/v2/bot/message/aggregation/list")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        );

    // Match query parameters if they exist
    if let Some(limit) = params.limit {
        mock_builder = mock_builder.match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".to_string(), limit.to_string())
        ]));
    }
    if let Some(start) = params.start {
        if let Some(limit) = params.limit {
            mock_builder = mock_builder.match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("limit".to_string(), limit.to_string()),
                Matcher::UrlEncoded("start".to_string(), start)
            ]));
        } else {
            mock_builder = mock_builder.match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start".to_string(), start)
            ]));
        }
    }

    mock_builder
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{LineOptions, error::Error, messaging_api::get_v2_bot_message_aggregation_list};

    use super::*;

    // cargo test --all-features test_make_mock_get_v2_bot_message_aggregation_list_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_message_aggregation_list_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.limit(Some(50u8));
        builder.custom_aggregation_units(vec!["unit1".to_string(), "unit2".to_string(), "unit3".to_string()]);
        builder.next(Some("next_token_123".to_string()));
        let mock = make_mock(&mut server, Some(builder)).await;

        let query_params = get_v2_bot_message_aggregation_list::QueryParams {
            limit: Some(50),
            start: None,
        };

        let res = get_v2_bot_message_aggregation_list::execute(
            &query_params,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.custom_aggregation_units, vec!["unit1", "unit2", "unit3"]);
        assert_eq!(res.0.next, Some("next_token_123".to_string()));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_message_aggregation_list_with_start -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_message_aggregation_list_with_start() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.limit(Some(100u8));
        builder.start(Some("start_token_456".to_string()));
        builder.custom_aggregation_units(vec!["unit4".to_string(), "unit5".to_string()]);
        let mock = make_mock(&mut server, Some(builder)).await;

        let query_params = get_v2_bot_message_aggregation_list::QueryParams {
            limit: Some(100),
            start: Some("start_token_456".to_string()),
        };

        let res = get_v2_bot_message_aggregation_list::execute(
            &query_params,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.custom_aggregation_units, vec!["unit4", "unit5"]);
        assert_eq!(res.0.next, None);

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_message_aggregation_list_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_message_aggregation_list_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(400usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let query_params = get_v2_bot_message_aggregation_list::QueryParams {
            limit: None,
            start: None,
        };

        let res = get_v2_bot_message_aggregation_list::execute(
            &query_params,
            "test_channel_access_token",
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