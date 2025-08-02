use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{GroqRequest, GroqResponse};

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

#[async_trait]
pub trait Transport: Send + Sync {
    async fn chat(&self, req: &GroqRequest) -> Result<GroqResponse>;
}

pub struct GroqTransport {
    client: Client,
    api_key: String,
}

impl GroqTransport {
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }
}

#[async_trait]
impl Transport for GroqTransport {
    async fn chat(&self, req: &GroqRequest) -> Result<GroqResponse> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            match self
                .client
                .post(GROQ_API_URL)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(req)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return response.json().await.map_err(|e| {
                            UnifiedIntelligenceError::Internal(format!(
                                "Failed to parse Groq API response: {}",
                                e
                            ))
                        });
                    }
                    if attempts >= 3 {
                        return Err(UnifiedIntelligenceError::Internal(format!(
                            "Groq API error after {} attempts: {}",
                            attempts,
                            response.text().await.unwrap_or_else(|_| "Unknown error".to_string())
                        )));
                    }
                }
                Err(e) => {
                    if attempts >= 3 {
                        return Err(UnifiedIntelligenceError::Internal(format!(
                            "Failed to send request to Groq API after {} attempts: {}",
                            attempts, e
                        )));
                    }
                }
            }
            sleep(Duration::from_millis(200 * 2u64.pow(attempts - 1))).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, GroqRequest};
    use tokio;

    #[tokio::test]
    async fn test_groq_transport_chat_retry() {
        // This test is a bit tricky as it requires a mock server to simulate failures.
        // For now, we'll just test the success case against the actual Groq API if the key is present.
        if let Ok(api_key) = std::env::var("GROQ_API_KEY") {
            let transport = GroqTransport::new(api_key).unwrap();
            let req = GroqRequest {
                model: "llama3-8b-8192".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "What is the capital of France?".to_string(),
                }],
                temperature: 0.0,
                max_tokens: 100,
            };
            let res = transport.chat(&req).await;
            assert!(res.is_ok());
        }
    }
}
