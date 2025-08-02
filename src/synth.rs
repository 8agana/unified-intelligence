use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{QueryIntent, Thought, GroqRequest, ChatMessage};
use crate::transport::Transport;

pub struct GroqSynth {
    tx: Arc<dyn Transport>,
    model_fast: String,
    model_deep: String,
}

impl GroqSynth {
    pub fn new(tx: Arc<dyn Transport>, model_fast: String, model_deep: String) -> Self {
        Self { tx, model_fast, model_deep }
    }
}

#[async_trait]
pub trait Synthesizer: Send + Sync {
    async fn synth(&self, intent: &QueryIntent, ctx: &[Thought]) -> Result<String>;
}

#[async_trait]
impl Synthesizer for GroqSynth {
    async fn synth(&self, intent: &QueryIntent, ctx: &[Thought]) -> Result<String> {
        tracing::info!("Synthesizing response with Groq for query: {}", intent.original_query);
        
        // Determine model based on synthesis style or default to fast
        let model = if let Some(style) = &intent.synthesis_style {
            match style.as_str() {
                "deep" => self.model_deep.clone(),
                _ => self.model_fast.clone(),
            }
        } else {
            self.model_fast.clone()
        };

        // Sort memories chronologically by default
        let mut sorted_memories = ctx.to_vec();
        sorted_memories.sort_by_key(|t| t.created_at);
        
        // Build context from sorted retrieved thoughts, enforcing token limits
        // This is a simplified token limit enforcement. A more robust solution would use a tokenizer.
        let mut context = String::new();
        let mut current_tokens = 0;
        const MAX_CONTEXT_TOKENS: usize = 1000; // Approximate token limit for context

        for thought in sorted_memories.iter().rev() { // Iterate in reverse to get newest first, but add oldest first to context
            let thought_str = format!("\nThought ID: {}\nContent: {}\nCreated At: {}", thought.id, thought.content, thought.created_at);
            // Simple approximation: 1 token ~ 4 characters
            let thought_tokens = thought_str.len() / 4;
            if current_tokens + thought_tokens > MAX_CONTEXT_TOKENS {
                break; // Stop if adding this thought exceeds the limit
            }
            context.insert_str(0, &thought_str); // Prepend to keep oldest first
            current_tokens += thought_tokens;
        }
        
        let system_message_content = if let Some(style) = &intent.synthesis_style {
            match style.to_lowercase().as_str() {
                "chronological" => r#"You are a helpful assistant that synthesizes information from retrieved memories to answer a query. Present the information in chronological order based on the 'created_at' timestamp of the memories. Do not include the raw memories in your response, only the synthesized answer. Be concise and directly answer the query based on the provided context."# .to_string(),
                _ => r#"You are a helpful assistant that synthesizes information from retrieved memories to answer a query. Do not include the raw memories in your response, only the synthesized answer. Be concise and directly answer the query based on the provided context."# .to_string(),
            }
        } else {
            r#"You are a helpful assistant that synthesizes information from retrieved memories to answer a query. Do not include the raw memories in your response, only the synthesized answer. Be concise and directly answer the query based on the provided context."# .to_string()
        };
        
        let system_message = ChatMessage {
            role: "system".to_string(),
            content: system_message_content,
        };
        
        let user_message = ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Original Query: {}\n\nRetrieved Memories:\n{}\n\nSynthesized Answer:",
                intent.original_query, context
            ),
        };
        
        let request = GroqRequest {
            model,
            messages: vec![system_message, user_message],
            temperature: 0.3,
            max_tokens: 1500, // Adjust as needed
            response_format: None, // No specific format needed for synthesis
        };
        
        let groq_response = self.tx.chat(&request).await?;
            
        if let Some(choice) = groq_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(UnifiedIntelligenceError::Internal(
                "Groq API returned empty choices".to_string()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Thought, QueryIntent, TemporalFilter, ChatMessage, GroqRequest, GroqResponse, Choice};
    use crate::transport::Transport;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use chrono::{Utc, Duration};
    use uuid::Uuid;

    // Mock Transport for testing
    struct MockTransport {
        responses: Mutex<Vec<GroqResponse>>,
    }

    impl MockTransport {
        fn new(responses: Vec<GroqResponse>) -> Self {
            MockTransport { responses: Mutex::new(responses) }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn chat(&self, _req: &GroqRequest) -> Result<GroqResponse> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(response) = responses.pop() {
                Ok(response)
            } else {
                Err(UnifiedIntelligenceError::Internal("No more mock responses".to_string()))
            }
        }
    }

    fn create_mock_thought(content: &str, days_ago: i64) -> Thought {
        Thought {
            id: Uuid::new_v4(),
            content: content.to_string(),
            category: None,
            tags: vec![],
            instance_id: "test_instance".to_string(),
            created_at: Utc::now() - Duration::days(days_ago),
            updated_at: Utc::now() - Duration::days(days_ago),
            importance: 5,
            relevance: 5,
            semantic_score: Some(0.8),
            temporal_score: None,
            usage_score: None,
            combined_score: None,
        }
    }

    #[tokio::test]
    async fn test_groq_synth_basic() {
        let mock_response = GroqResponse {
            choices: vec![Choice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Synthesized response content.".to_string(),
                },
            }],
        };
        let mock_transport = MockTransport::new(vec![mock_response]);
        let groq_synth = GroqSynth::new(Arc::new(mock_transport), "fast-model".to_string(), "deep-model".to_string());

        let intent = QueryIntent {
            original_query: "Test query.".to_string(),
            temporal_filter: None,
            synthesis_style: None,
        };
        let thoughts = vec![
            create_mock_thought("Thought 1", 1),
            create_mock_thought("Thought 2", 2),
        ];

        let result = groq_synth.synth(&intent, &thoughts).await.unwrap();
        assert_eq!(result, "Synthesized response content.");
    }

    #[tokio::test]
    async fn test_groq_synth_token_truncation() {
        let mock_response = GroqResponse {
            choices: vec![Choice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Synthesized response content.".to_string(),
                },
            }],
        };
        let mock_transport = MockTransport::new(vec![mock_response]);
        let groq_synth = GroqSynth::new(Arc::new(mock_transport), "fast-model".to_string(), "deep-model".to_string());

        let intent = QueryIntent {
            original_query: "Test query.".to_string(),
            temporal_filter: None,
            synthesis_style: None,
        };

        // Create many thoughts to exceed token limit
        let mut thoughts = Vec::new();
        for i in 0..50 {
            thoughts.push(create_mock_thought(&format!("This is a very long thought content number {}. ", i).repeat(50), i));
        }

        let result = groq_synth.synth(&intent, &thoughts).await.unwrap();
        assert_eq!(result, "Synthesized response content.");
        // Further assertions could check the actual context length passed to the mock transport
    }

    #[tokio::test]
    async fn test_groq_synth_deep_model() {
        let mock_response = GroqResponse {
            choices: vec![Choice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Deep synthesized response content.".to_string(),
                },
            }],
        };
        let mock_transport = MockTransport::new(vec![mock_response]);
        let groq_synth = GroqSynth::new(Arc::new(mock_transport), "fast-model".to_string(), "deep-model".to_string());

        let intent = QueryIntent {
            original_query: "Test query.".to_string(),
            temporal_filter: None,
            synthesis_style: Some("deep".to_string()),
        };
        let thoughts = vec![create_mock_thought("Thought 1", 1)];

        let result = groq_synth.synth(&intent, &thoughts).await.unwrap();
        assert_eq!(result, "Deep synthesized response content.");
    }
}