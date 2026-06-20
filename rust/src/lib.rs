//! OAuth 2.0 + PKCE for Google Cloud / Vertex AI.
//!
//! No service account required — authenticates the end-user directly via
//! Google OAuth 2.0 with PKCE. Supports token exchange, auto-refresh,
//! revocation, project listing, and SSE streaming.
//!
//! # Example
//!
//! ```rust,no_run
//! use vertex_ai_oauth::{OAuthCredentials, generate_pkce, build_auth_url, exchange_code};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let creds = OAuthCredentials {
//!     client_id: "your-client-id".into(),
//!     client_secret: None,
//!     redirect_uri: "com.example://oauth2callback".into(),
//! };
//! let pkce = generate_pkce();
//! let auth_url = build_auth_url(&creds, Some(&pkce));
//! // ... user authenticates, receives `code` ...
//! let client = reqwest::Client::new();
//! let tokens = exchange_code(&client, &creds, "auth-code", Some(&pkce.verifier)).await?;
//! # Ok(())
//! # }
//! ```

mod auth;
mod api;
mod error;

pub use auth::*;
pub use api::*;
pub use error::*;
