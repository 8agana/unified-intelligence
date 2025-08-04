use async_trait::async_trait;
use rand::Rng;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{GroqRequest, GroqResponse};

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const MAX_RETRIES: u8 = 5;
const MAX_RETRY_DURATION: Duration = Duration::from_secs(300); // 5 minutes max

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
        let start_time = Instant::now();
        let mut attempts = 0;

        while attempts < MAX_RETRIES {
            // Check if we've exceeded the maximum retry duration
            if start_time.elapsed() > MAX_RETRY_DURATION {
                return Err(UnifiedIntelligenceError::Internal(format!(
                    "Groq API request timed out after {} seconds (max retry duration exceeded)",
                    MAX_RETRY_DURATION.as_secs()
                )));
            }

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
                                "Failed to parse Groq API response: {e}"
                            ))
                        });
                    }

                    // For non-success responses, return error after max attempts
                    if attempts >= MAX_RETRIES {
                        return Err(UnifiedIntelligenceError::Internal(format!(
                            "Groq API error after {} attempts: {}",
                            attempts,
                            response
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unknown error".to_string())
                        )));
                    }
                }
                Err(e) => {
                    // For network errors, return error after max attempts
                    if attempts >= MAX_RETRIES {
                        return Err(UnifiedIntelligenceError::Internal(format!(
                            "Failed to send request to Groq API after {attempts} attempts: {e}"
                        )));
                    }
                }
            }

            // Exponential backoff with jitter (only if we're going to retry)
            if attempts < MAX_RETRIES {
                let base_delay =
                    Duration::from_millis(200 * 2u64.pow(attempts.saturating_sub(1) as u32));
                let jitter = rand::thread_rng().gen_range(0.8..=1.2);
                let delay = Duration::from_millis((base_delay.as_millis() as f64 * jitter) as u64);

                // Cap the delay to prevent excessive waiting
                let max_delay = Duration::from_secs(30);
                let final_delay = std::cmp::min(delay, max_delay);

                sleep(final_delay).await;
            }
        }

        // This should never be reached due to the loop condition, but just in case
        Err(UnifiedIntelligenceError::Internal(format!(
            "Groq API request failed after {MAX_RETRIES} attempts"
        )))
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
            let transport = match GroqTransport::new(api_key) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to create transport in test: {e}");
                    return;
                }
            };
            let req = GroqRequest {
                model: "llama3-8b-8192".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "What is the capital of France?".to_string(),
                }],
                temperature: 0.0,
                max_tokens: 100,
                response_format: None,
            };
            let res = transport.chat(&req).await;
            assert!(res.is_ok());
        }
    }
}
