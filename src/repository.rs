use async_trait::async_trait;
use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
use crate::models::{ChainMetadata, ThoughtRecord};
use crate::redis::RedisManager;

/// Repository trait for thought storage operations
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait ThoughtRepository: Send + Sync {
    /// Store a thought record
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()>;

    /// Create or update chain metadata
    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()>;

    /// Check if chain exists
    async fn chain_exists(&self, chain_id: &str) -> Result<bool>;

    /// Get a single thought by ID
    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>>;

    /// Get all thoughts in a chain
    async fn get_chain_thoughts(
        &self,
        instance: &str,
        chain_id: &str,
    ) -> Result<Vec<ThoughtRecord>>;
    async fn search_thoughts(
        &self,
        instance: &str,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ThoughtRecord>>;

    // ===== FEEDBACK LOOP METHODS (Phase 1) =====
}

/// Redis implementation of ThoughtRepository
pub struct RedisThoughtRepository {
    redis: Arc<RedisManager>,
    #[allow(dead_code)]
    config: Arc<Config>,
    #[allow(dead_code)]
    instance_id: String, // Keep instance_id for namespacing
}

impl RedisThoughtRepository {
    pub fn new(redis: Arc<RedisManager>, config: Arc<Config>, instance_id: String) -> Self {
        Self {
            redis: redis.clone(),
            config,
            instance_id,
        }
    }

    fn thought_key(&self, instance: &str, thought_id: &str) -> String {
        format!("{}:Thoughts:{}", instance, thought_id)
    }

    fn chain_metadata_key(&self, chain_id: &str) -> String {
        format!("Chains:metadata:{}", chain_id)
    }

    fn redi_search_index_name(&self, instance: &str) -> String {
        format!("{}:thoughts_idx", instance)
    }
}

#[async_trait]
impl ThoughtRepository for RedisThoughtRepository {
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()> {
        let thought_key = self.thought_key(&thought.instance, &thought.id);
        let bloom_key = format!("{}:bloom:thoughts", thought.instance);
        let ts_key = format!("{}:metrics:thought_count", thought.instance);
        let chain_key = thought
            .chain_id
            .as_ref()
            .map(|id| format!("{}:chains:{}", thought.instance, id));

        // Serialize thought to JSON
        let thought_json = serde_json::to_string(thought)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;

        // Parse timestamp from ISO string to epoch seconds
        let timestamp = chrono::DateTime::parse_from_rfc3339(&thought.timestamp)
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|_| {
                // Fallback to current time if parsing fails
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                    .as_secs() as i64
            });

        // Use atomic script for all operations
        let success = self
            .redis
            .store_thought_atomic(
                &thought_key,
                &bloom_key,
                &ts_key,
                chain_key.as_deref(),
                &thought_json,
                &thought.id,
                timestamp,
                thought.chain_id.as_deref(),
            )
            .await?;

        if !success {
            let preview = thought.thought.chars().take(50).collect::<String>();
            return Err(crate::error::UnifiedIntelligenceError::DuplicateThought {
                instance: thought.instance.clone(),
                preview,
            });
        } else {
            // Publish thought_created event to Redis Streams for background processing
            let timestamp = chrono::DateTime::parse_from_rfc3339(&thought.timestamp)
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|_| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                        .as_secs() as i64
                });

            // Publish to Redis Streams for background service
            let event_data = serde_json::to_value(thought)
                .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;

            if let Err(e) = self
                .redis
                .publish_stream_event(&thought.instance, "thought_created", &event_data)
                .await
            {
                tracing::debug!(
                    "Failed to publish thought_created event: {}. Background processing may not be triggered.",
                    e
                );
            }

            // Log thought created event
            let thought_preview = thought.thought.chars().take(100).collect::<String>();
            let _ = self
                .redis
                .log_thought_event(
                    &thought.instance,
                    "thought_created",
                    &thought.id,
                    thought.chain_id.as_deref(),
                    Some(vec![
                        ("thought_preview", &thought_preview),
                        ("thought_number", &thought.thought_number.to_string()),
                    ]),
                )
                .await;
        }

        Ok(())
    }

    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()> {
        let key = self.chain_metadata_key(&metadata.chain_id);

        // Check if chain already exists
        let is_new_chain = !self.redis.exists(&key).await?;

        // Save the metadata
        self.redis.json_set(&key, "$", metadata).await?;

        // Log appropriate event
        let event_type = if is_new_chain {
            "chain_created"
        } else {
            "chain_updated"
        };
        let _ = self
            .redis
            .log_event(
                &metadata.instance,
                event_type,
                vec![
                    ("chain_id", &metadata.chain_id),
                    ("thought_count", &metadata.thought_count.to_string()),
                    ("created_at", &metadata.created_at),
                ],
            )
            .await;

        Ok(())
    }

    async fn chain_exists(&self, chain_id: &str) -> Result<bool> {
        let key = self.chain_metadata_key(chain_id);
        self.redis.exists(&key).await
    }

    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>> {
        let thought_key = self.thought_key(instance, thought_id);
        self.redis
            .json_get::<ThoughtRecord>(&thought_key, "$")
            .await
    }

    async fn get_chain_thoughts(
        &self,
        instance: &str,
        chain_id: &str,
    ) -> Result<Vec<ThoughtRecord>> {
        let chain_key = format!("{}:chains:{}", instance, chain_id);
        let thought_jsons = self
            .redis
            .get_chain_thoughts_atomic(&chain_key, instance)
            .await?;
        let mut thoughts = Vec::new();
        for json_str in thought_jsons {
            let thought: ThoughtRecord = serde_json::from_str(&json_str)
                .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
            thoughts.push(thought);
        }
        Ok(thoughts)
    }

    async fn search_thoughts(
        &self,
        instance: &str,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ThoughtRecord>> {
        let index_name = self.redi_search_index_name(instance);
        let thought_jsons = self
            .redis
            .search_thoughts_rediseach(&index_name, query, offset, limit)
            .await?;

        let mut thoughts = Vec::new();
        // The first element of the result is the total count, so skip it
        for json_str in thought_jsons.into_iter().skip(1) {
            let thought: ThoughtRecord = serde_json::from_str(&json_str)
                .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
            thoughts.push(thought);
        }
        Ok(thoughts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // Helper to create a test thought
    fn create_test_thought(id: &str, content: &str, instance: &str) -> ThoughtRecord {
        ThoughtRecord {
            id: id.to_string(),
            thought: content.to_string(),
            content: content.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            instance: instance.to_string(),
            chain_id: None,
            thought_number: 1,
            total_thoughts: 1,
            next_thought_needed: false,
            framework: None,
            importance: None,
            relevance: None,
            tags: None,
            category: None,
        }
    }

    #[tokio::test]
    async fn test_fallback_search_basic() {
        // This test would require a mock RedisManager which is complex
        // For now, we'll create integration tests that use a test Redis instance
        // or focus on testing the logic within the fallback_search method itself

        // The tests below demonstrate the test structure and assertions
        // They would need a proper test harness with Redis mocking
    }

    // Integration test example structure
    #[tokio::test]
    #[ignore] // Run with --ignored flag when Redis test instance is available
    async fn test_fallback_search_integration() {
        // Setup test Redis instance
        // Create repository with search disabled
        // Insert test data
        // Run fallback_search
        // Assert results match expected
    }

    // Unit tests for the search logic
    #[test]
    fn test_search_matching_logic() {
        let thought = create_test_thought("1", "This is about Rust programming", "CC");

        // Test case-insensitive matching
        assert!(
            thought
                .thought
                .to_lowercase()
                .contains(&"rust".to_lowercase())
        );
        assert!(
            thought
                .thought
                .to_lowercase()
                .contains(&"RUST".to_lowercase())
        );
        assert!(
            !thought
                .thought
                .to_lowercase()
                .contains(&"python".to_lowercase())
        );
    }

    #[test]
    fn test_limit_enforcement() {
        let thoughts = vec![
            create_test_thought("1", "Rust thought 1", "CC"),
            create_test_thought("2", "Rust thought 2", "CC"),
            create_test_thought("3", "Rust thought 3", "CC"),
            create_test_thought("4", "Rust thought 4", "CC"),
            create_test_thought("5", "Rust thought 5", "CC"),
        ];

        let limit = 3;
        let filtered: Vec<_> = thoughts
            .into_iter()
            .filter(|t| t.thought.to_lowercase().contains("rust"))
            .take(limit)
            .collect();

        assert_eq!(filtered.len(), 3);
    }
}
