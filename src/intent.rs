use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{ChatMessage, GroqRequest, QueryIntent};
use crate::transport::Transport;

pub struct GroqIntent {
    tx: Arc<dyn Transport>,
    model: String,
}

impl GroqIntent {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(tx: Arc<dyn Transport>, model: String) -> Self {
        Self { tx, model }
    }
}

#[async_trait]
pub trait IntentParser: Send + Sync {
    #[cfg_attr(not(test), allow(dead_code))]
    async fn parse(&self, query: &str) -> Result<QueryIntent>;
}

#[async_trait]
impl IntentParser for GroqIntent {
    async fn parse(&self, query: &str) -> Result<QueryIntent> {
        tracing::info!("Parsing query intent with Groq for query: {}", query);

        let system_message = ChatMessage {
            role: "system".to_string(),
            content: r#"You are an intent parser. Your task is to extract specific information from a user's natural language query and return it as a JSON object.
The JSON object should have the following structure:
{
    "original_query": "string", // The original query from the user, rephrased to remove temporal or synthesis style requests
    "temporal_filter": { // Optional: Information about temporal filtering
        "start_date": "YYYY-MM-DD", // Optional: Specific start date
        "end_date": "YYYY-MM-DD",   // Optional: Specific end date
        "relative_timeframe": "string" // Optional: e.g., "yesterday", "last week", "last month", "last year", "past 3 days"
    },
    "synthesis_style": "string" // Optional: "chronological" or "one_block"
}

If a field is not explicitly mentioned or implied in the user's query, omit it from the JSON.
For dates, use YYYY-MM-DD format. If a relative timeframe is given, provide that string.
For synthesis_style, if the user asks for a timeline, step-by-step, or ordered output, use "chronological". Otherwise, omit it.

Examples:
User: "What did Gem and Sam do yesterday?"
Output:
{
    "original_query": "What did Gem and Sam do?",
    "temporal_filter": {
        "relative_timeframe": "yesterday"
    }
}

User: "Summarize my thoughts from last week in chronological order."
Output:
{
    "original_query": "Summarize my thoughts.",
    "temporal_filter": {
        "relative_timeframe": "last week"
    },
    "synthesis_style": "chronological"
}

User: "Tell me about my thoughts on Rust."
Output:
{
    "original_query": "Tell me about my thoughts on Rust."
}

User: "What happened on 2023-01-15?"
Output:
{
    "original_query": "What happened?",
    "temporal_filter": {
        "start_date": "2023-01-15",
        "end_date": "2023-01-15"
    }
}
"#.to_string(),
        };

        let user_message = ChatMessage {
            role: "user".to_string(),
            content: format!("User Query: {query}"),
        };

        let request = GroqRequest {
            model: self.model.clone(),
            messages: vec![system_message, user_message],
            temperature: 0.0, // Keep temperature low for consistent JSON output
            max_tokens: 1500,
            response_format: Some(serde_json::json!({"type": "json_object"})),
        };

        let groq_response = self.tx.chat(&request).await?;

        if let Some(choice) = groq_response.choices.first() {
            let json_string = choice.message.content.clone();
            serde_json::from_str(&json_string).map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to deserialize Groq intent JSON: {e}. Raw: {json_string}"
                ))
            })
        } else {
            Err(UnifiedIntelligenceError::Internal(
                "Groq API returned empty choices for intent parsing".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, Choice, GroqResponse};
    use crate::transport::Transport;
    use async_trait::async_trait;
    use std::sync::Mutex;

    // Mock Transport for testing
    struct MockTransport {
        responses: Mutex<Vec<GroqResponse>>,
    }

    impl MockTransport {
        fn new(responses: Vec<GroqResponse>) -> Self {
            MockTransport {
                responses: Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn chat(&self, _req: &GroqRequest) -> Result<GroqResponse> {
            let mut responses = self
                .responses
                .lock()
                .expect("Mock transport mutex should not be poisoned");
            if let Some(response) = responses.pop() {
                Ok(response)
            } else {
                Err(UnifiedIntelligenceError::Internal(
                    "No more mock responses".to_string(),
                ))
            }
        }
    }

    #[tokio::test]
    async fn test_groq_intent_parse() {
        let mock_response = GroqResponse {
            choices: vec![Choice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: r#"{
                        "original_query": "What did Gem and Sam do?",
                        "temporal_filter": {
                            "relative_timeframe": "yesterday"
                        },
                        "synthesis_style": "chronological"
                    }"#
                    .to_string(),
                },
            }],
        };
        let mock_transport = MockTransport::new(vec![mock_response]);
        let groq_intent = GroqIntent::new(Arc::new(mock_transport), "test-model".to_string());

        let query_intent = groq_intent
            .parse("What did Gem and Sam do yesterday in chronological order?")
            .await
            .expect("Intent parsing should succeed in test");

        assert_eq!(query_intent.original_query, "What did Gem and Sam do?");
        assert_eq!(
            query_intent
                .temporal_filter
                .expect("Should have temporal filter")
                .relative_timeframe
                .expect("Should have relative timeframe"),
            "yesterday"
        );
        assert_eq!(
            query_intent
                .synthesis_style
                .expect("Should have synthesis style"),
            "chronological"
        );
    }
}
