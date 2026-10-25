use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("provider {provider} failed: {message}")]
    Provider { provider: String, message: String },
    #[error("no provider available for role {0}: all failed or are over budget")]
    NoProvider(String),
    #[error("model reply could not be parsed: {0}")]
    Parse(String),
    #[error("missing setting: {0}")]
    Config(String),
    #[error("external tool `{tool}` failed: {message}")]
    Tool { tool: String, message: String },
    #[error("publish failed: {0}")]
    Publish(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
