use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OAuthError;

#[derive(Debug, Clone, Serialize)]
pub struct GenerateRequest {
    pub contents: Vec<Content>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(rename = "safetySettings")]
    pub safety_settings: Vec<SafetySetting>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GenerationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: u32,
    pub temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct StreamChunk {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Candidate {
    content: Option<Content>,
}

pub fn default_safety_settings() -> Vec<SafetySetting> {
    [
        "HARM_CATEGORY_SEXUALLY_EXPLICIT",
        "HARM_CATEGORY_HATE_SPEECH",
        "HARM_CATEGORY_HARASSMENT",
        "HARM_CATEGORY_DANGEROUS_CONTENT",
        "HARM_CATEGORY_CIVIC_INTEGRITY",
    ]
    .iter()
    .map(|cat| SafetySetting {
        category: cat.to_string(),
        threshold: "BLOCK_NONE".into(),
    })
    .collect()
}

pub fn build_endpoint(project_id: &str, region: &str, model: &str) -> String {
    let host = if region == "global" {
        "aiplatform.googleapis.com".to_string()
    } else {
        format!("{}-aiplatform.googleapis.com", region)
    };
    format!(
        "https://{}/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
        host, project_id, region, model
    )
}

pub async fn stream_generate(
    client: &reqwest::Client,
    access_token: &str,
    project_id: &str,
    region: &str,
    model: &str,
    request: &GenerateRequest,
    on_chunk: impl Fn(&str),
) -> Result<String, OAuthError> {
    let url = build_endpoint(project_id, region, model);

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(request)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::Api { status, body });
    }

    let mut full_text = String::new();
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| OAuthError::Http(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(json_str) {
                    if let Some(candidates) = &chunk.candidates {
                        for candidate in candidates {
                            if let Some(content) = &candidate.content {
                                for part in &content.parts {
                                    if part.thought != Some(true) {
                                        if let Some(text) = &part.text {
                                            full_text.push_str(text);
                                            on_chunk(text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(full_text)
}

pub async fn generate_non_streaming(
    client: &reqwest::Client,
    access_token: &str,
    project_id: &str,
    region: &str,
    model: &str,
    request: &GenerateRequest,
) -> Result<String, OAuthError> {
    let host = if region == "global" {
        "aiplatform.googleapis.com".to_string()
    } else {
        format!("{}-aiplatform.googleapis.com", region)
    };
    let url = format!(
        "https://{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        host, project_id, region, model
    );

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(request)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::Api { status, body });
    }

    let body: Value = resp.json().await?;
    let text = body
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(text)
}

pub async fn list_models(
    client: &reqwest::Client,
    access_token: &str,
    project_id: &str,
    region: &str,
) -> Result<Vec<String>, OAuthError> {
    let host = if region == "global" {
        "aiplatform.googleapis.com".to_string()
    } else {
        format!("{}-aiplatform.googleapis.com", region)
    };
    let url = format!(
        "https://{}/v1/projects/{}/locations/{}/publishers/google/models",
        host, project_id, region
    );

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

    let body: Value = resp.json().await?;
    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    m.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n.rsplit('/').next().unwrap_or(n).to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}
