use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
    pub details: Vec<ErrorDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub message: String,
    pub property: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO {0}")]
    IO(#[from] std::io::Error),

    #[error("Timeout")]
    Timeout,

    #[error("Upload {0}")]
    Upload(String),

    #[error("Other {0}")]
    Other(String, StatusCode),

    #[error("reqwest {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("serde json {0}")]
    Json(#[from] serde_json::Error),

    #[error("Line {0:?} {1}")]
    Line(ErrorResponse, StatusCode),
}
