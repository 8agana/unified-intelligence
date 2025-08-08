use async_trait::async_trait;
use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
use crate::models::{ChainMetadata, ThoughtRecord};
use crate::redis::RedisManager;
use crate::repository_traits::ThoughtRepository;

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

// ========== KNOWLEDGE GRAPH REPOSITORY IMPLEMENTATION ==========

use crate::models::{KnowledgeNode, KnowledgeRelation, KnowledgeScope, EntityType};
use crate::repository_traits::KnowledgeRepository;
use redis::{RedisError, Script};

pub struct RedisKnowledgeRepository {
    redis_manager: Arc<RedisManager>,
    // Atomic script for entity creation + index update
    create_entity_script: Script,
}

impl RedisKnowledgeRepository {
    pub fn new(redis_manager: Arc<RedisManager>) -> Self {
        // Lua script for atomic entity creation + index update
        let create_entity_script = Script::new(r#"
            local entity_key = KEYS[1]
            local index_key = KEYS[2]
            local entity_json = ARGV[1]
            local entity_name = ARGV[2]
            local entity_id = ARGV[3]
            
            -- Create entity
            redis.call('JSON.SET', entity_key, '$', entity_json)
            
            -- Update name index atomically using HSET
            redis.call('HSET', index_key, entity_name, entity_id)
            
            return 'OK'
        "#);
        
        Self { redis_manager, create_entity_script }
    }
    
    // Use Display trait instead of Debug format for keys
    fn get_entity_key(&self, id: &str, scope: &KnowledgeScope) -> String {
        format!("{}:KG:entity:{}", scope, id)
    }
    
    fn get_relation_key(&self, id: &str, scope: &KnowledgeScope) -> String {
        format!("{}:KG:relation:{}", scope, id)
    }
    
    // Use Redis Hash for name index
    fn get_index_key(&self, scope: &KnowledgeScope) -> String {
        format!("{}:KG:index:name_to_id", scope)
    }
    
    fn get_relation_index_key(&self, entity_id: &str, scope: &KnowledgeScope) -> String {
        format!("{}:KG:index:entity_relations:{}", scope, entity_id)
    }
    
    // SCAN-based search as fallback (non-blocking)
    async fn search_entities_with_scan(
        &self,
        query: &str,
        scope: &KnowledgeScope,
        entity_type: Option<&EntityType>,
        limit: usize
    ) -> Result<Vec<KnowledgeNode>> {
        let mut conn = self.redis_manager.get_connection().await?;
        let pattern = format!("{}:KG:entity:*", scope);
        let mut cursor = 0u64;
        let mut results = Vec::new();
        
        // Use SCAN instead of KEYS to avoid blocking
        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100) // Process in batches of 100
                .query_async(&mut conn)
                .await
                .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
            
            for key in keys {
                if let Ok(json_str) = redis::cmd("JSON.GET")
                    .arg(&key)
                    .arg("$")
                    .query_async::<String>(&mut conn)
                    .await
                {
                    if let Ok(nodes) = serde_json::from_str::<Vec<KnowledgeNode>>(&json_str) {
                        for node in nodes {
                            // Filter by query (check name, tags, and display_name)
                            let matches_query = node.name.to_lowercase().contains(&query.to_lowercase()) 
                                || node.display_name.to_lowercase().contains(&query.to_lowercase())
                                || node.tags.iter().any(|tag| tag.to_lowercase().contains(&query.to_lowercase()));
                            
                            // Filter by entity type if specified
                            let matches_type = entity_type.map_or(true, |et| {
                                match (&node.entity_type, et) {
                                    (EntityType::Custom(a), EntityType::Custom(b)) => a == b,
                                    _ => std::mem::discriminant(&node.entity_type) == std::mem::discriminant(et)
                                }
                            });
                            
                            if matches_query && matches_type {
                                results.push(node);
                                if results.len() >= limit {
                                    return Ok(results);
                                }
                            }
                        }
                    }
                }
            }
            
            cursor = new_cursor;
            if cursor == 0 {
                break; // Scan complete
            }
        }
        
        Ok(results)
    }
    
    // Missing relation index implementation
    async fn update_relation_index(
        &self, 
        entity_id: &str, 
        relation_id: &str, 
        direction: &str, 
        scope: &KnowledgeScope
    ) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        let index_key = self.get_relation_index_key(entity_id, scope);
        
        // Use Redis Hash for relation indices
        let field = format!("{}:{}", direction, relation_id);
        let _: () = redis::AsyncCommands::hset(&mut conn, &index_key, &field, chrono::Utc::now().to_rfc3339()).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        Ok(())
    }
}

#[async_trait]
impl KnowledgeRepository for RedisKnowledgeRepository {
    async fn create_entity(&self, node: KnowledgeNode) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        let entity_key = self.get_entity_key(&node.id, &node.scope);
        let index_key = self.get_index_key(&node.scope);
        let json_str = serde_json::to_string(&node)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
        
        // Use atomic Lua script for entity creation + index update
        let _: String = self.create_entity_script
            .key(entity_key)
            .key(index_key)
            .arg(&json_str)
            .arg(&node.name)
            .arg(&node.id)
            .invoke_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        // Knowledge graph entities should persist indefinitely (no TTL)
        
        tracing::info!("Created knowledge entity '{}' in {} scope", node.name, node.scope);
        Ok(())
    }
    
    async fn get_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<KnowledgeNode> {
        let mut conn = self.redis_manager.get_connection().await?;
        let key = self.get_entity_key(id, scope);
        
        let json_str: String = redis::cmd("JSON.GET")
            .arg(&key)
            .arg("$")
            .query_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        // Parse the JSON array response (RedisJSON returns array even for single path)
        let json_array: Vec<KnowledgeNode> = serde_json::from_str(&json_str)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
        json_array.into_iter().next()
            .ok_or_else(|| crate::error::UnifiedIntelligenceError::NotFound(format!("Entity {} not found", id)))
    }
    
    async fn get_entity_by_name(&self, name: &str, scope: &KnowledgeScope) -> Result<KnowledgeNode> {
        let mut conn = self.redis_manager.get_connection().await?;
        let index_key = self.get_index_key(scope);
        
        // Use Redis Hash HGET instead of JSON.GET for name index
        let entity_id: Option<String> = redis::AsyncCommands::hget(&mut conn, &index_key, name).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        let entity_id = entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::NotFound(format!("Entity '{}' not found in index", name))
        })?;
        
        self.get_entity(&entity_id, scope).await
    }
    
    async fn update_entity(&self, node: KnowledgeNode) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        let entity_key = self.get_entity_key(&node.id, &node.scope);
        
        // Update the entire entity
        let json_str = serde_json::to_string(&node)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
        
        let _: String = redis::cmd("JSON.SET")
            .arg(&entity_key)
            .arg("$")
            .arg(&json_str)
            .query_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        tracing::info!("Updated knowledge entity '{}'", node.name);
        Ok(())
    }
    
    async fn delete_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        
        // Get entity first to get the name for index cleanup
        let entity = self.get_entity(id, scope).await?;
        let entity_key = self.get_entity_key(id, scope);
        let index_key = self.get_index_key(scope);
        
        // Delete entity
        let _: i32 = redis::cmd("JSON.DEL")
            .arg(&entity_key)
            .query_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        // Remove from name index
        let _: () = redis::AsyncCommands::hdel(&mut conn, &index_key, &entity.name).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        tracing::info!("Deleted knowledge entity '{}' from {} scope", entity.name, scope);
        Ok(())
    }
    
    async fn search_entities(
        &self,
        query: &str,
        scope: &KnowledgeScope,
        entity_type: Option<&EntityType>,
        limit: usize
    ) -> Result<Vec<KnowledgeNode>> {
        tracing::info!("Searching for '{}' in {} scope", query, scope);
        
        // Use SCAN-based search (production-safe)
        let entities = self.search_entities_with_scan(query, scope, entity_type, limit).await?;
        
        Ok(entities)
    }
    
    async fn create_relation(&self, relation: KnowledgeRelation) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        let relation_key = self.get_relation_key(&relation.id, &relation.scope);
        let json_str = serde_json::to_string(&relation)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?;
        
        // Store relation
        let _: String = redis::cmd("JSON.SET")
            .arg(&relation_key)
            .arg("$")
            .arg(&json_str)
            .query_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        // Update relation indices for both entities
        self.update_relation_index(&relation.from_entity_id, &relation.id, "outgoing", &relation.scope).await?;
        self.update_relation_index(&relation.to_entity_id, &relation.id, "incoming", &relation.scope).await?;
        
        tracing::info!("Created relation '{}' from {} to {}", 
            relation.relationship_type, relation.from_entity_id, relation.to_entity_id);
        Ok(())
    }
    
    async fn get_relations(&self, entity_id: &str, scope: &KnowledgeScope) -> Result<Vec<KnowledgeRelation>> {
        let mut conn = self.redis_manager.get_connection().await?;
        let index_key = self.get_relation_index_key(entity_id, scope);
        
        // Get all relations from the index
        let relations: std::collections::HashMap<String, String> = redis::AsyncCommands::hgetall(&mut conn, &index_key).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        let mut result = Vec::new();
        for (field, _) in relations {
            if let Some(relation_id) = field.split(':').nth(1) {
                let relation_key = self.get_relation_key(relation_id, scope);
                
                if let Ok(json_str) = redis::cmd("JSON.GET")
                    .arg(&relation_key)
                    .arg("$")
                    .query_async::<String>(&mut conn)
                    .await
                {
                    if let Ok(rels) = serde_json::from_str::<Vec<KnowledgeRelation>>(&json_str) {
                        result.extend(rels);
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    async fn update_name_index(&self, name: &str, id: &str, scope: &KnowledgeScope) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        let index_key = self.get_index_key(scope);
        
        // Use Redis Hash HSET for atomic index updates
        let _: () = redis::AsyncCommands::hset(&mut conn, &index_key, name, id).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        Ok(())
    }
    
    async fn set_active_entity(&self, session_key: &str, entity_id: &str, scope: &KnowledgeScope) -> Result<()> {
        let mut conn = self.redis_manager.get_connection().await?;
        
        let value = serde_json::json!({
            "entity_id": entity_id,
            "scope": scope,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        let _: () = redis::AsyncCommands::set_ex(&mut conn, session_key, serde_json::to_string(&value)
            .map_err(|e| crate::error::UnifiedIntelligenceError::Json(e))?, 3600).await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        Ok(())
    }
    
    async fn add_thought_to_entity(&self, entity_name: &str, thought_id: &str, scope: &KnowledgeScope) -> Result<()> {
        // Get entity by name first
        let entity = self.get_entity_by_name(entity_name, scope).await?;
        let entity_key = self.get_entity_key(&entity.id, scope);
        
        let mut conn = self.redis_manager.get_connection().await?;
        
        // Append thought_id to the entity's thought_ids array
        let _: i32 = redis::cmd("JSON.ARRAPPEND")
            .arg(&entity_key)
            .arg("$.thought_ids")
            .arg(&format!("\"{}\"", thought_id))
            .query_async(&mut conn)
            .await
            .map_err(|e: RedisError| crate::error::UnifiedIntelligenceError::Redis(e))?;
        
        tracing::info!("Added thought {} to entity '{}'", thought_id, entity_name);
        Ok(())
    }
}

/// Combined repository that implements both ThoughtRepository and KnowledgeRepository
pub struct CombinedRedisRepository {
    thought_repo: RedisThoughtRepository,
    knowledge_repo: RedisKnowledgeRepository,
}

impl CombinedRedisRepository {
    pub fn new(redis_manager: Arc<RedisManager>, config: Arc<Config>, instance_id: String) -> Self {
        let thought_repo = RedisThoughtRepository::new(redis_manager.clone(), config, instance_id);
        let knowledge_repo = RedisKnowledgeRepository::new(redis_manager);
        
        Self {
            thought_repo,
            knowledge_repo,
        }
    }
}

#[async_trait]
impl ThoughtRepository for CombinedRedisRepository {
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()> {
        self.thought_repo.save_thought(thought).await
    }

    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()> {
        self.thought_repo.save_chain_metadata(metadata).await
    }

    async fn chain_exists(&self, chain_id: &str) -> Result<bool> {
        self.thought_repo.chain_exists(chain_id).await
    }

    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>> {
        self.thought_repo.get_thought(instance, thought_id).await
    }

    async fn get_chain_thoughts(
        &self,
        instance: &str,
        chain_id: &str,
    ) -> Result<Vec<ThoughtRecord>> {
        self.thought_repo.get_chain_thoughts(instance, chain_id).await
    }

    async fn search_thoughts(
        &self,
        instance: &str,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ThoughtRecord>> {
        self.thought_repo.search_thoughts(instance, query, offset, limit).await
    }
}

#[async_trait]
impl KnowledgeRepository for CombinedRedisRepository {
    async fn create_entity(&self, node: KnowledgeNode) -> Result<()> {
        self.knowledge_repo.create_entity(node).await
    }

    async fn get_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<KnowledgeNode> {
        self.knowledge_repo.get_entity(id, scope).await
    }

    async fn get_entity_by_name(&self, name: &str, scope: &KnowledgeScope) -> Result<KnowledgeNode> {
        self.knowledge_repo.get_entity_by_name(name, scope).await
    }

    async fn update_entity(&self, node: KnowledgeNode) -> Result<()> {
        self.knowledge_repo.update_entity(node).await
    }

    async fn delete_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<()> {
        self.knowledge_repo.delete_entity(id, scope).await
    }

    async fn search_entities(
        &self, 
        query: &str, 
        scope: &KnowledgeScope,
        entity_type: Option<&EntityType>,
        limit: usize
    ) -> Result<Vec<KnowledgeNode>> {
        self.knowledge_repo.search_entities(query, scope, entity_type, limit).await
    }

    async fn create_relation(&self, relation: KnowledgeRelation) -> Result<()> {
        self.knowledge_repo.create_relation(relation).await
    }

    async fn get_relations(&self, entity_id: &str, scope: &KnowledgeScope) -> Result<Vec<KnowledgeRelation>> {
        self.knowledge_repo.get_relations(entity_id, scope).await
    }

    async fn update_name_index(&self, name: &str, id: &str, scope: &KnowledgeScope) -> Result<()> {
        self.knowledge_repo.update_name_index(name, id, scope).await
    }

    async fn set_active_entity(&self, session_key: &str, entity_id: &str, scope: &KnowledgeScope) -> Result<()> {
        self.knowledge_repo.set_active_entity(session_key, entity_id, scope).await
    }

    async fn add_thought_to_entity(&self, entity_name: &str, thought_id: &str, scope: &KnowledgeScope) -> Result<()> {
        self.knowledge_repo.add_thought_to_entity(entity_name, thought_id, scope).await
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
