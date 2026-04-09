use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpotError {
    #[error("not authenticated — run parrotui-spotify to start OAuth flow")]
    NotAuthenticated,

    #[error("token expired and refresh failed: {0}")]
    TokenRefreshFailed(String),

    #[error("spotify api error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("playback engine failed: {0}")]
    Playback(String),

    #[error("librespot session error: {0}")]
    Session(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("keyring: {0}")]
    Keyring(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, SpotError>;
