use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetail>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub message: String,
    pub property: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Timeout")]
    Timeout,

    #[error("Other {0}")]
    Other(String, StatusCode),

    #[error("reqwest {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("serde json {0}")]
    Json(#[from] serde_json::Error),

    #[error("Line {0:?} {1}")]
    Line(ErrorResponse, StatusCode),
}

impl Error {
    pub fn make_json(&self) -> serde_json::Value {
        match self {
            Error::Line(response, _) => {
                serde_json::to_value(response).unwrap()
            }
            Error::Other(messages, status_code ) => {
                serde_json::json!({
                    "message": messages,
                    "status_code": status_code.as_u16()
                })
            }
            _ => {
                serde_json::json!({
                    "message": self.to_string()
                })
            }
        }
    }
}
