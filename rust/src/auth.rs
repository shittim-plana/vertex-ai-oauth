use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::OAuthError;

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const REVOKE_ENDPOINT: &str = "https://oauth2.googleapis.com/revoke";
const GCP_PROJECTS_ENDPOINT: &str = "https://cloudresourcemanager.googleapis.com/v1/projects";

pub const DEFAULT_SCOPE: &str =
    "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/cloudplatformprojects.readonly";

const REFRESH_MARGIN: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
}

#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpProject {
    #[serde(rename = "projectId")]
    pub project_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

pub fn generate_pkce() -> PkceChallenge {
    use rand::Rng;
    let verifier: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let hash = Sha256::digest(verifier.as_bytes());
    let challenge = base64url_encode(&hash);
    PkceChallenge { verifier, challenge }
}

fn base64url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

pub fn build_auth_url(creds: &OAuthCredentials, pkce: Option<&PkceChallenge>) -> String {
    build_auth_url_with_scope(creds, pkce, DEFAULT_SCOPE)
}

pub fn build_auth_url_with_scope(
    creds: &OAuthCredentials,
    pkce: Option<&PkceChallenge>,
    scope: &str,
) -> String {
    let mut params = vec![
        ("client_id", creds.client_id.as_str()),
        ("redirect_uri", creds.redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", scope),
        ("access_type", "offline"),
        ("prompt", "select_account consent"),
    ];
    let challenge_str;
    if let Some(p) = pkce {
        challenge_str = p.challenge.clone();
        params.push(("code_challenge", &challenge_str));
        params.push(("code_challenge_method", "S256"));
    }
    let query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, uri_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", AUTH_ENDPOINT, query)
}

pub async fn exchange_code(
    client: &reqwest::Client,
    creds: &OAuthCredentials,
    code: &str,
    code_verifier: Option<&str>,
) -> Result<Tokens, OAuthError> {
    let mut params = vec![
        ("code".to_string(), code.to_string()),
        ("client_id".to_string(), creds.client_id.clone()),
        ("redirect_uri".to_string(), creds.redirect_uri.clone()),
        ("grant_type".to_string(), "authorization_code".to_string()),
    ];
    if let Some(secret) = &creds.client_secret {
        params.push(("client_secret".to_string(), secret.clone()));
    }
    if let Some(verifier) = code_verifier {
        params.push(("code_verifier".to_string(), verifier.to_string()));
    }

    let resp = client.post(TOKEN_ENDPOINT).form(&params).send().await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::OAuth(body));
    }

    let token_resp: TokenResponse = resp.json().await?;
    Ok(to_tokens(token_resp))
}

pub async fn refresh_token(
    client: &reqwest::Client,
    creds: &OAuthCredentials,
    refresh: &str,
) -> Result<Tokens, OAuthError> {
    let mut params = vec![
        ("refresh_token".to_string(), refresh.to_string()),
        ("client_id".to_string(), creds.client_id.clone()),
        ("grant_type".to_string(), "refresh_token".to_string()),
    ];
    if let Some(secret) = &creds.client_secret {
        params.push(("client_secret".to_string(), secret.clone()));
    }

    let resp = client.post(TOKEN_ENDPOINT).form(&params).send().await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("invalid_grant") {
            return Err(OAuthError::InvalidGrant);
        }
        return Err(OAuthError::OAuth(body));
    }

    let mut token_resp: TokenResponse = resp.json().await?;
    if token_resp.refresh_token.is_none() {
        token_resp.refresh_token = Some(refresh.to_string());
    }
    Ok(to_tokens(token_resp))
}

pub async fn get_valid_token(
    client: &reqwest::Client,
    creds: &OAuthCredentials,
    tokens: &Tokens,
) -> Result<Tokens, OAuthError> {
    if !tokens.needs_refresh() {
        return Ok(tokens.clone());
    }
    let refresh = tokens
        .refresh_token
        .as_deref()
        .ok_or(OAuthError::OAuth("no refresh token".into()))?;
    refresh_token(client, creds, refresh).await
}

pub async fn revoke_token(client: &reqwest::Client, token: &str) -> Result<(), OAuthError> {
    let resp = client
        .post(REVOKE_ENDPOINT)
        .form(&[("token", token)])
        .send()
        .await?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::Http(format!("token revoke failed: {body}")));
    }
    Ok(())
}

pub async fn list_gcp_projects(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<GcpProject>, OAuthError> {
    let url = format!("{}?filter=lifecycleState:ACTIVE", GCP_PROJECTS_ENDPOINT);
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::Api { status, body });
    }

    let body: serde_json::Value = resp.json().await?;
    let projects = body
        .get("projects")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(GcpProject {
                        project_id: p.get("projectId")?.as_str()?.to_string(),
                        name: p.get("name")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(projects)
}

impl Tokens {
    pub fn is_expired(&self) -> bool {
        now_secs() >= self.expires_at
    }

    pub fn needs_refresh(&self) -> bool {
        now_secs() + REFRESH_MARGIN.as_secs() >= self.expires_at
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn to_tokens(resp: TokenResponse) -> Tokens {
    Tokens {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        expires_at: now_secs() + resp.expires_in,
    }
}

pub fn uri_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_verifier_length() {
        let pkce = generate_pkce();
        assert_eq!(pkce.verifier.len(), 64);
        assert!(!pkce.challenge.is_empty());
    }

    #[test]
    fn auth_url_contains_required_params() {
        let creds = OAuthCredentials {
            client_id: "test-client".into(),
            client_secret: None,
            redirect_uri: "http://localhost/cb".into(),
        };
        let url = build_auth_url(&creds, None);
        assert!(url.starts_with(AUTH_ENDPOINT));
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=select_account%20consent"));
    }

    #[test]
    fn auth_url_with_pkce() {
        let creds = OAuthCredentials {
            client_id: "test".into(),
            client_secret: None,
            redirect_uri: "com.test://cb".into(),
        };
        let pkce = generate_pkce();
        let url = build_auth_url(&creds, Some(&pkce));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn token_expiration() {
        let expired = Tokens {
            access_token: "t".into(),
            refresh_token: None,
            expires_at: 0,
        };
        assert!(expired.is_expired());
        assert!(expired.needs_refresh());

        let fresh = Tokens {
            access_token: "t".into(),
            refresh_token: None,
            expires_at: u64::MAX,
        };
        assert!(!fresh.is_expired());
        assert!(!fresh.needs_refresh());
    }
}
