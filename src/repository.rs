use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;
use crate::models::{ThoughtRecord, ChainMetadata, ThoughtMetadata};
use crate::redis::RedisManager;
use crate::config::Config;

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
    async fn get_chain_thoughts(&self, instance: &str, chain_id: &str) -> Result<Vec<ThoughtRecord>>;
    
    // ===== FEEDBACK LOOP METHODS (Phase 1) =====
    
    /// Save thought metadata for feedback loop system
    async fn save_thought_metadata(&self, metadata: &ThoughtMetadata) -> Result<()>;
    
    /// Publish event to feedback stream for background processing
    async fn publish_feedback_event(&self, event: &serde_json::Value) -> Result<()>;
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
    pub fn new(
        redis: Arc<RedisManager>, 
        config: Arc<Config>,
        instance_id: String,
    ) -> Self {
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
}

#[async_trait]
impl ThoughtRepository for RedisThoughtRepository {
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()> {
        let thought_key = self.thought_key(&thought.instance, &thought.id);
        let bloom_key = format!("{}:bloom:thoughts", thought.instance);
        let ts_key = format!("{}:metrics:thought_count", thought.instance);
        let chain_key = thought.chain_id.as_ref()
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
                    .unwrap()
                    .as_secs() as i64
            });
        
        // Use atomic script for all operations
        let success = self.redis.store_thought_atomic(
            &thought_key,
            &bloom_key,
            &ts_key,
            chain_key.as_deref(),
            &thought_json,
            &thought.id,
            timestamp,
            thought.chain_id.as_deref(),
        ).await?;
        
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
                        .unwrap()
                        .as_secs() as i64
                });
            
            // Publish to Redis Streams for background service
            let event_data = serde_json::json!({
                "type": "thought_created",
                "thought_id": thought.id,
                "instance": thought.instance,
                "timestamp": timestamp,
                "content_preview": thought.thought.chars().take(100).collect::<String>()
            });
            
            if let Err(e) = self.redis.publish_stream_event(&thought.instance, "thought_created", &event_data).await {
                tracing::debug!("Failed to publish thought_created event: {}. Background processing may not be triggered.", e);
            }
            
            // Log thought created event
            let thought_preview = thought.thought.chars().take(100).collect::<String>();
            let _ = self.redis.log_thought_event(
                &thought.instance,
                "thought_created",
                &thought.id,
                thought.chain_id.as_deref(),
                Some(vec![
                    ("thought_preview", &thought_preview),
                    ("thought_number", &thought.thought_number.to_string()),
                ]),
            ).await;
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
        let event_type = if is_new_chain { "chain_created" } else { "chain_updated" };
        let _ = self.redis.log_event(
            &metadata.instance,
            event_type,
            vec![
                ("chain_id", &metadata.chain_id),
                ("thought_count", &metadata.thought_count.to_string()),
                ("created_at", &metadata.created_at),
            ],
        ).await;
        
        Ok(())
    }
    
    
    async fn chain_exists(&self, chain_id: &str) -> Result<bool> {
        let key = self.chain_metadata_key(chain_id);
        self.redis.exists(&key).await
    }
    
    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>> {
        let thought_key = self.thought_key(instance, thought_id);
        self.redis.json_get::<ThoughtRecord>(&thought_key, "$").await
    }

    async fn get_chain_thoughts(&self, instance: &str, chain_id: &str) -> Result<Vec<ThoughtRecord>> {
        let chain_key = format!("{}:chains:{}", instance, chain_id);
        let thought_jsons = self.redis.get_chain_thoughts_atomic(&chain_key, instance).await?;
        let mut thoughts = Vec::new();
        for json_str in thought_jsons {
            let thought: ThoughtRecord = serde_json::from_str(&json_str)
                .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
            thoughts.push(thought);
        }
        Ok(thoughts)
    }
    
    // ===== FEEDBACK LOOP IMPLEMENTATIONS (Phase 1) =====
    
    async fn save_thought_metadata(&self, metadata: &ThoughtMetadata) -> Result<()> {
        let key = format!("{}:thought_meta:{}", metadata.instance, metadata.thought_id);
        
        // Store metadata as JSON
        let metadata_json = serde_json::to_string(metadata)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
        
        self.redis.json_set(&key, ".", &serde_json::from_str::<serde_json::Value>(&metadata_json)?).await?;
        
        // Build tag indexes if tags are provided
        if let Some(ref tags) = metadata.tags {
            for tag in tags {
                let tag_key = format!("{}:tags:{}", metadata.instance, tag);
                self.redis.sadd(&tag_key, &metadata.thought_id).await?;
            }
        }
        
        tracing::debug!("Saved metadata for thought {} in instance {}", metadata.thought_id, metadata.instance);
        Ok(())
    }
    
    
    async fn publish_feedback_event(&self, event: &serde_json::Value) -> Result<()> {
        // Extract instance from event or use a default stream
        let instance = event.get("instance")
            .and_then(|v| v.as_str())
            .unwrap_or("global");
        
        let stream_key = format!("{}:feedback_events", instance);
        
        // Convert JSON object to Redis Stream fields with owned strings
        let mut field_pairs = Vec::new();
        if let Some(obj) = event.as_object() {
            for (key, value) in obj {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                field_pairs.push((key.clone(), value_str));
            }
        }
        
        // Convert to string references for Redis command
        let fields: Vec<(&str, &str)> = field_pairs.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        let field_count = fields.len();
        
        // Publish to Redis Stream using XADD
        self.redis.xadd(&stream_key, "*", fields).await?;
        
        tracing::debug!("Published feedback event to stream {} with {} fields", stream_key, field_count);
        Ok(())
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
            similarity: None,
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
        assert!(thought.thought.to_lowercase().contains(&"rust".to_lowercase()));
        assert!(thought.thought.to_lowercase().contains(&"RUST".to_lowercase()));
        assert!(!thought.thought.to_lowercase().contains(&"python".to_lowercase()));
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
        let filtered: Vec<_> = thoughts.into_iter()
            .filter(|t| t.thought.to_lowercase().contains("rust"))
            .take(limit)
            .collect();
            
        assert_eq!(filtered.len(), 3);
    }
}