use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::messaging_api::LineResponseHeader;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetail>>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorDetail {
    pub message: String,
    pub property: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid {0}")]
    Invalid(String),

    #[error("Other {0}")]
    OtherText(String, StatusCode, LineResponseHeader),

    #[error("OtherJson {0}")]
    OtherJson(serde_json::Value, StatusCode, LineResponseHeader),

    #[error("reqwest {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("serde json {0}")]
    Json(#[from] serde_json::Error),

    #[error("Line {0:?} {1}")]
    Line(ErrorResponse, StatusCode, LineResponseHeader),
}

impl Error {
    pub fn status_code(&self) -> Option<StatusCode> {
        match self {
            Error::Line(_, status_code, _) => Some(*status_code),
            Error::OtherJson(_, status_code, _) => Some(*status_code),
            Error::OtherText(_, status_code, _) => Some(*status_code),
            _ => None,
        }
    }

    pub fn make_json(&self) -> serde_json::Value {
        match self {
            Error::Line(response, status_code, line_header) => {
                serde_json::json!({
                    "response": response,
                    "status_code": status_code.as_u16(),
                    "line_header": line_header
                })
            }
            Error::OtherJson(json, status_code, line_header) => {
                serde_json::json!({
                    "json": json,
                    "status_code": status_code.as_u16(),
                    "line_header": line_header
                })
            }
            Error::OtherText(messages, status_code, line_header) => {
                serde_json::json!({
                    "message": messages,
                    "status_code": status_code.as_u16(),
                    "line_header": line_header
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
