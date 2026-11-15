use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlathubError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Failed to parse JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("HTTP Error {status}: {message}")]
    Http { status: u16, message: String },

    #[error("Not Found (404)")]
    NotFound,
}
