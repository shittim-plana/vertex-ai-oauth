use std::fmt;

#[derive(Debug)]
pub enum OAuthError {
    Http(String),
    OAuth(String),
    InvalidGrant,
    Api { status: u16, body: String },
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(msg) => write!(f, "HTTP error: {}", msg),
            Self::OAuth(msg) => write!(f, "OAuth error: {}", msg),
            Self::InvalidGrant => write!(f, "invalid_grant: re-authentication required"),
            Self::Api { status, body } => write!(f, "API error {}: {}", status, body),
        }
    }
}

impl std::error::Error for OAuthError {}

impl From<reqwest::Error> for OAuthError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}

impl From<serde_json::Error> for OAuthError {
    fn from(e: serde_json::Error) -> Self {
        Self::Http(e.to_string())
    }
}
