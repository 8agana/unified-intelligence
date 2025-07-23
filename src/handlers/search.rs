use serde_json::json;
use tracing;

use crate::error::Result;
use crate::models::{UiSearchParams, SearchResponse};
use crate::repository::ThoughtRepository;

/// Trait for search-related operations
pub trait SearchHandler {
    /// Handle ui_search tool - unified search for thoughts and chains
    async fn ui_search(&self, params: UiSearchParams) -> Result<SearchResponse>;
}

impl<R: ThoughtRepository> SearchHandler for super::ToolHandlers<R> {
    /// Handle ui_search tool - unified search for thoughts and chains
    async fn ui_search(&self, params: UiSearchParams) -> Result<SearchResponse> {
        let limit = params.limit.unwrap_or(50);
        let search_all_instances = params.search_all_instances.unwrap_or(false);
        
        // Generate search ID for tracking
        let search_id = self.repository.generate_search_id().await?;
        
        // Check if metadata filters are applied
        let has_metadata_filters = params.tags_filter.is_some() || 
            params.min_importance.is_some() || 
            params.min_relevance.is_some() || 
            params.category_filter.is_some();
        
        // Determine search mode based on flattened parameters
        let (results, search_method) = match params.mode.as_str() {
            "chain" => {
                let chain_id = params.chain_id
                    .ok_or_else(|| crate::error::UnifiedIntelligenceError::Validation {
                        field: "chain_id".to_string(),
                        reason: "chain_id is required when mode is 'chain'".to_string()
                    })?;
                
                tracing::info!(
                    "Chain search for instance '{}', chain_id: {}, search_id: {}", 
                    self.instance_id, chain_id, search_id
                );
                
                let thoughts = self.repository.get_chain_thoughts(&self.instance_id, &chain_id).await?;
                (thoughts, "chain_search".to_string())
            },
            "thought" => {
                let query = params.query
                    .ok_or_else(|| crate::error::UnifiedIntelligenceError::Validation {
                        field: "query".to_string(),
                        reason: "query is required when mode is 'thought'".to_string()
                    })?;
                
                tracing::info!(
                    "Thought search for instance '{}', query: {}, global: {}, search_id: {}",
                    self.instance_id, query, search_all_instances, search_id
                );
                
                // Use text search only
                let thoughts = if search_all_instances {
                    self.repository.search_thoughts_global(&query, limit).await?
                } else {
                    self.repository.search_thoughts(&self.instance_id, &query, limit).await?
                };
                
                let method = "text_search".to_string();
                
                (thoughts, method)
            },
            _ => {
                return Err(crate::error::UnifiedIntelligenceError::Validation {
                    field: "mode".to_string(),
                    reason: format!("Invalid search mode: '{}'. Must be 'chain' or 'thought'", params.mode)
                });
            }
        };
        
        let total_found = results.len();
        
        // Publish search event for tracking
        let search_event = json!({
            "event_type": "search_performed",
            "search_id": search_id,
            "instance": self.instance_id,
            "search_mode": &params.mode,
            "results_count": total_found,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Err(e) = self.repository.publish_feedback_event(&search_event).await {
            tracing::warn!("Failed to publish search event: {}", e);
        }
        
        Ok(SearchResponse {
            results,
            total_found,
            search_method,
            search_id,
        })
    }
}