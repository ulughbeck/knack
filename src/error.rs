use thiserror::Error;

pub type Result<T> = std::result::Result<T, KnackError>;

#[derive(Debug, Error)]
pub enum KnackError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("jsonc error: {0}")]
    Jsonc(#[from] json5::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
}

impl KnackError {
    pub fn msg(message: impl Into<String>) -> Self {
        KnackError::Message(message.into())
    }
}
