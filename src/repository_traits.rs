use async_trait::async_trait;
use crate::error::Result;
use crate::models::{ThoughtRecord, ChainMetadata, Identity, ThoughtMetadata};
use crate::identity_documents::{IdentityDocument, IdentityMetadata};

/// Read operations for thoughts
#[async_trait]
pub trait ThoughtReader: Send + Sync {
    /// Get a thought by ID
    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>>;
    
    /// Get thoughts by chain ID
    async fn get_chain_thoughts(&self, instance: &str, chain_id: &str) -> Result<Vec<ThoughtRecord>>;
    
    /// Search thoughts by query
    async fn search_thoughts(
        &self,
        instance: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ThoughtRecord>>;
    
    /// Get all thoughts for an instance
    async fn get_instance_thoughts(&self, instance: &str, limit: usize) -> Result<Vec<ThoughtRecord>>;
    
    /// Search thoughts across all instances
    async fn search_thoughts_global(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ThoughtRecord>>;
    
    /// Get chain metadata
    async fn get_chain_metadata(&self, chain_id: &str) -> Result<Option<ChainMetadata>>;
    
    /// Check if chain exists
    async fn chain_exists(&self, chain_id: &str) -> Result<bool>;
    
    /// Get thought metadata
    async fn get_thought_metadata(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtMetadata>>;
    
    /// Get thoughts by tag intersection
    async fn get_thoughts_by_tags(&self, instance: &str, tags: &[String]) -> Result<Vec<String>>;
    
    /// Get boost score for a thought
    async fn get_boost_score(&self, instance: &str, thought_id: &str) -> Result<f64>;
    
    /// Get top boosted thoughts
    async fn get_top_boosted_thoughts(&self, instance: &str, limit: usize) -> Result<Vec<(String, f64)>>;
}

/// Write operations for thoughts
#[async_trait]
pub trait ThoughtWriter: Send + Sync {
    /// Store a thought record
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()>;
    
    /// Save chain metadata
    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()>;
    
    /// Save thought metadata
    async fn save_thought_metadata(&self, metadata: &ThoughtMetadata) -> Result<()>;
    
    /// Update boost score
    async fn update_boost_score(
        &self,
        instance: &str,
        thought_id: &str,
        feedback_action: &str,
        relevance_rating: Option<i32>,
        dwell_time: Option<i32>
    ) -> Result<f64>;
    
    /// Publish feedback event
    async fn publish_feedback_event(&self, event: &serde_json::Value) -> Result<()>;
    
    /// Log event to instance stream
    async fn log_event(&self, instance: &str, event_type: &str, fields: Vec<(&str, &str)>) -> Result<()>;
}

/// Operations for identity management
#[async_trait]
pub trait IdentityRepository: Send + Sync {
    /// Get identity
    async fn get_identity(&self, identity_key: &str) -> Result<Option<Identity>>;
    
    /// Save identity
    async fn save_identity(&self, identity_key: &str, identity: &Identity) -> Result<()>;
    
    /// Delete identity
    async fn delete_identity(&self, identity_key: &str) -> Result<()>;
    
    /// Get identity documents by field
    async fn get_identity_documents_by_field(&self, instance_id: &str, field_type: &str) -> Result<Vec<IdentityDocument>>;
    
    /// Save identity document
    async fn save_identity_document(&self, document: &IdentityDocument) -> Result<()>;
    
    /// Delete identity document
    async fn delete_identity_document(&self, instance_id: &str, field_type: &str, document_id: &str) -> Result<()>;
    
    /// Get all identity documents
    async fn get_all_identity_documents(&self, instance_id: &str) -> Result<Vec<IdentityDocument>>;
    
    /// Search identity documents
    async fn search_identity_documents(&self, instance_id: &str, query: &str, limit: Option<usize>) -> Result<Vec<IdentityDocument>>;
    
    /// Get identity document by ID
    async fn get_identity_document_by_id(&self, instance_id: &str, document_id: &str) -> Result<Option<IdentityDocument>>;
    
    /// Update identity document metadata
    async fn update_identity_document_metadata(&self, instance_id: &str, document_id: &str, metadata: IdentityMetadata) -> Result<()>;
    
    /// Migrate monolithic identity to documents
    async fn migrate_identity_to_documents(&self, instance_id: &str) -> Result<Vec<IdentityDocument>>;
}

/// JSON operations trait
#[async_trait]
pub trait JsonOperations: Send + Sync {
    /// Append to JSON array
    async fn json_array_append(&self, key: &str, path: &str, value: &serde_json::Value) -> Result<()>;
    
    /// Set JSON field
    async fn json_set(&self, key: &str, path: &str, value: &serde_json::Value) -> Result<()>;
    
    /// Get JSON array
    async fn json_get_array(&self, key: &str, path: &str) -> Result<Option<Vec<serde_json::Value>>>;
    
    /// Delete JSON field
    async fn json_delete(&self, key: &str, path: &str) -> Result<()>;
    
    /// Increment JSON numeric field
    async fn json_increment(&self, key: &str, path: &str, increment: i64) -> Result<()>;
}

/// Combined repository trait that includes all operations
#[async_trait]
pub trait ThoughtRepository: ThoughtReader + ThoughtWriter + IdentityRepository + JsonOperations {
    /// Generate unique search ID
    async fn generate_search_id(&self) -> Result<String>;
    
    /// Apply boost scores to search results
    async fn apply_boost_scores(&self, instance: &str, thoughts: &mut Vec<ThoughtRecord>) -> Result<()>;
}