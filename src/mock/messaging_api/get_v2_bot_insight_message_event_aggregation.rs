use derive_builder::Builder;
use mockito::{Matcher, Mock, Server};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockParams {
    pub channel_access_token: String,
    pub custom_aggregation_unit: String,
    pub from: String,
    pub to: String,
    pub status_code: usize,
    pub unique_impression: Option<u64>,
    pub unique_click: Option<u64>,
    pub unique_media_played: Option<u64>,
    pub unique_media_played_100_percent: Option<u64>,
    pub messages: Vec<MockMessage>,
    pub clicks: Vec<MockClick>,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockMessage {
    pub seq: u64,
    pub impression: Option<u64>,
    pub unique_impression: Option<u64>,
    pub media_played: Option<u64>,
    pub media_played_25_percent: Option<u64>,
    pub media_played_50_percent: Option<u64>,
    pub media_played_75_percent: Option<u64>,
    pub media_played_100_percent: Option<u64>,
    pub unique_media_played: Option<u64>,
    pub unique_media_played_25_percent: Option<u64>,
    pub unique_media_played_50_percent: Option<u64>,
    pub unique_media_played_75_percent: Option<u64>,
    pub unique_media_played_100_percent: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Builder, Default)]
#[builder(setter(into))]
#[builder(default)]
#[builder(field(public))]
pub struct MockClick {
    pub seq: u64,
    pub url: Option<String>,
    pub click: Option<u64>,
    pub unique_click: Option<u64>,
    pub unique_click_of_request: Option<u64>,
}

pub async fn make_mock(server: &mut Server, builder: Option<MockParamsBuilder>) -> Mock {
    let mut builder = builder.unwrap_or_default();
    if builder.channel_access_token.is_none() {
        builder.channel_access_token("test_channel_access_token".to_string());
    }
    if builder.custom_aggregation_unit.is_none() {
        builder.custom_aggregation_unit("promotion_a".to_string());
    }
    if builder.from.is_none() {
        builder.from("20240801".to_string());
    }
    if builder.to.is_none() {
        builder.to("20240831".to_string());
    }
    if builder.status_code.is_none() {
        builder.status_code(200usize);
    }
    if builder.unique_impression.is_none() {
        builder.unique_impression(Some(1000u64));
    }
    if builder.unique_click.is_none() {
        builder.unique_click(Some(150u64));
    }
    if builder.unique_media_played.is_none() {
        builder.unique_media_played(Some(75u64));
    }
    if builder.unique_media_played_100_percent.is_none() {
        builder.unique_media_played_100_percent(Some(50u64));
    }
    if builder.messages.is_none() {
        let default_message = MockMessage {
            seq: 1,
            impression: Some(500),
            unique_impression: Some(400),
            media_played: Some(100),
            media_played_25_percent: Some(90),
            media_played_50_percent: Some(80),
            media_played_75_percent: Some(70),
            media_played_100_percent: Some(60),
            unique_media_played: Some(50),
            unique_media_played_25_percent: Some(45),
            unique_media_played_50_percent: Some(40),
            unique_media_played_75_percent: Some(35),
            unique_media_played_100_percent: Some(30),
        };
        builder.messages(vec![default_message]);
    }
    if builder.clicks.is_none() {
        let default_click = MockClick {
            seq: 1,
            url: Some("https://example.com".to_string()),
            click: Some(200),
            unique_click: Some(180),
            unique_click_of_request: Some(160),
        };
        builder.clicks(vec![default_click]);
    }
    if builder.error_message.is_none() {
        builder.error_message("error occurred".to_string());
    }
    let params = builder.build().unwrap();

    let body_json = if params.status_code == 200 {
        let messages_json: Vec<serde_json::Value> = params
            .messages
            .iter()
            .map(|msg| {
                json!({
                    "seq": msg.seq,
                    "impression": msg.impression,
                    "uniqueImpression": msg.unique_impression,
                    "mediaPlayed": msg.media_played,
                    "mediaPlayed25Percent": msg.media_played_25_percent,
                    "mediaPlayed50Percent": msg.media_played_50_percent,
                    "mediaPlayed75Percent": msg.media_played_75_percent,
                    "mediaPlayed100Percent": msg.media_played_100_percent,
                    "uniqueMediaPlayed": msg.unique_media_played,
                    "uniqueMediaPlayed25Percent": msg.unique_media_played_25_percent,
                    "uniqueMediaPlayed50Percent": msg.unique_media_played_50_percent,
                    "uniqueMediaPlayed75Percent": msg.unique_media_played_75_percent,
                    "uniqueMediaPlayed100Percent": msg.unique_media_played_100_percent,
                })
            })
            .collect();

        let clicks_json: Vec<serde_json::Value> = params
            .clicks
            .iter()
            .map(|click| {
                json!({
                    "seq": click.seq,
                    "url": click.url,
                    "click": click.click,
                    "uniqueClick": click.unique_click,
                    "uniqueClickOfRequest": click.unique_click_of_request,
                })
            })
            .collect();

        json!({
            "overview": {
                "uniqueImpression": params.unique_impression,
                "uniqueClick": params.unique_click,
                "uniqueMediaPlayed": params.unique_media_played,
                "uniqueMediaPlayed100Percent": params.unique_media_played_100_percent,
            },
            "messages": messages_json,
            "clicks": clicks_json,
        })
    } else {
        json!({
            "message": params.error_message
        })
    };

    server
        .mock("GET", "/v2/bot/insight/message/event/aggregation")
        .match_header(
            "authorization",
            format!("Bearer {}", params.channel_access_token).as_str(),
        )
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "customAggregationUnit".to_string(),
                params.custom_aggregation_unit,
            ),
            Matcher::UrlEncoded("from".to_string(), params.from),
            Matcher::UrlEncoded("to".to_string(), params.to),
        ]))
        .with_status(params.status_code)
        .with_header("content-type", "application/json")
        .with_body(body_json.to_string())
        .create_async()
        .await
}

#[cfg(test)]
mod tests {
    use crate::{
        LineOptions, error::Error, messaging_api::get_v2_bot_insight_message_event_aggregation,
    };

    use super::*;

    // cargo test --all-features test_make_mock_get_v2_bot_insight_message_event_aggregation_success -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_insight_message_event_aggregation_success() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.custom_aggregation_unit("test_unit".to_string());
        builder.from("20240701".to_string());
        builder.to("20240731".to_string());
        builder.unique_impression(Some(2000u64));
        builder.unique_click(Some(300u64));

        let test_message = MockMessage {
            seq: 2,
            impression: Some(1000),
            unique_impression: Some(800),
            media_played: Some(200),
            media_played_25_percent: Some(180),
            media_played_50_percent: Some(160),
            media_played_75_percent: Some(140),
            media_played_100_percent: Some(120),
            unique_media_played: Some(100),
            unique_media_played_25_percent: Some(90),
            unique_media_played_50_percent: Some(80),
            unique_media_played_75_percent: Some(70),
            unique_media_played_100_percent: Some(60),
        };
        builder.messages(vec![test_message]);

        let test_click = MockClick {
            seq: 2,
            url: Some("https://test.com".to_string()),
            click: Some(400),
            unique_click: Some(350),
            unique_click_of_request: Some(300),
        };
        builder.clicks(vec![test_click]);

        let mock = make_mock(&mut server, Some(builder)).await;

        let query_params = get_v2_bot_insight_message_event_aggregation::QueryParams {
            custom_aggregation_unit: "test_unit".to_string(),
            from: "20240701".to_string(),
            to: "20240731".to_string(),
        };

        let res = get_v2_bot_insight_message_event_aggregation::execute(
            &query_params,
            "test_channel_access_token",
            &LineOptions {
                prefix_url: Some(server.url()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(res.0.overview.unique_impression, Some(2000));
        assert_eq!(res.0.overview.unique_click, Some(300));
        assert_eq!(res.0.messages.len(), 1);
        assert_eq!(res.0.messages[0].seq, 2);
        assert_eq!(res.0.messages[0].impression, Some(1000));
        assert_eq!(res.0.clicks.len(), 1);
        assert_eq!(res.0.clicks[0].seq, 2);
        assert_eq!(res.0.clicks[0].url, Some("https://test.com".to_string()));

        mock.assert_async().await;
    }

    // cargo test --all-features test_make_mock_get_v2_bot_insight_message_event_aggregation_failure -- --nocapture --test-threads=1
    #[tokio::test]
    async fn test_make_mock_get_v2_bot_insight_message_event_aggregation_failure() {
        let mut server = Server::new_async().await;
        let mut builder = MockParamsBuilder::default();
        builder.status_code(404usize);
        let mock = make_mock(&mut server, Some(builder)).await;

        let query_params = get_v2_bot_insight_message_event_aggregation::QueryParams {
            custom_aggregation_unit: "promotion_a".to_string(),
            from: "20240801".to_string(),
            to: "20240831".to_string(),
        };

        let res = get_v2_bot_insight_message_event_aggregation::execute(
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
                assert_eq!(status_code, 404);
                assert_eq!(response.message, "error occurred");
            }
            _ => panic!("Unexpected response"),
        }

        mock.assert_async().await;
    }
}
