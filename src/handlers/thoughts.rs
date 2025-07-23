use serde_json::json;
use tracing;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{
    UiThinkParams, ThoughtRecord, ThinkResponse, ChainMetadata, ThoughtMetadata
};
use crate::repository::ThoughtRepository;
use crate::frameworks::{ThinkingFramework, FrameworkProcessor, FrameworkVisual};

/// Trait for thought-related operations
pub trait ThoughtsHandler {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse>;
}

impl<R: ThoughtRepository> ThoughtsHandler for super::ToolHandlers<R> {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse> {
        // Determine framework with validation
        let framework = if let Some(ref framework_str) = params.framework {
            match ThinkingFramework::from_string(framework_str) {
                Ok(f) => f,
                Err(e) => {
                    self.visual.error(&format!("Framework error: {}", e));
                    return Err(UnifiedIntelligenceError::Validation {
                        field: "framework".to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        } else {
            ThinkingFramework::Sequential
        };

        // Display visual start with framework
        self.visual.thought_start(params.thought_number, params.total_thoughts);
        FrameworkVisual::display_framework_start(&framework);
        self.visual.thought_content(&params.thought);
        
        // Process through framework
        if framework != ThinkingFramework::Sequential {
            let processor = FrameworkProcessor::new(framework.clone());
            let result = processor.process_thought(&params.thought, params.thought_number);
            
            FrameworkVisual::display_insights(&result.insights);
            FrameworkVisual::display_prompts(&result.prompts);
        }
        
        // Validate input
        self.validator.validate_thought_content(&params.thought)?;
        self.validator.validate_thought_numbers(params.thought_number, params.total_thoughts)?;
        if let Some(chain_id) = &params.chain_id {
            self.validator.validate_chain_id(chain_id)?;
        }
        
        tracing::info!(
            "Processing thought {} of {} for instance '{}'", 
            params.thought_number, 
            params.total_thoughts,
            self.instance_id
        );
        
        // Create thought record
        let thought = ThoughtRecord::new(
            self.instance_id.clone(),
            params.thought,
            params.thought_number,
            params.total_thoughts,
            params.chain_id.clone(),
            params.next_thought_needed,
        );
        
        let thought_id = thought.id.clone();
        
        // Handle chain metadata and visual display
        let _is_new_chain = if let Some(ref chain_id) = params.chain_id {
            let chain_exists = self.repository.chain_exists(chain_id).await?;
            if !chain_exists {
                let metadata = ChainMetadata {
                    chain_id: chain_id.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    thought_count: params.total_thoughts,
                    instance: self.instance_id.clone(),
                };
                self.repository.save_chain_metadata(&metadata).await?;
            }
            self.visual.chain_info(chain_id, !chain_exists);
            !chain_exists
        } else {
            false
        };
        
        // Save thought
        self.repository.save_thought(&thought).await?;
        
        // Save metadata if any new fields are provided (Phase 1 feedback loop implementation)
        if params.importance.is_some() || params.relevance.is_some() || 
           params.tags.is_some() || params.category.is_some() {
            let metadata = ThoughtMetadata::new(
                thought_id.clone(),
                self.instance_id.clone(),
                params.importance,
                params.relevance,
                params.tags.clone(),
                params.category.clone(),
            );
            
            // Store metadata in Redis using pattern: {instance}:thought_meta:{id}
            self.repository.save_thought_metadata(&metadata).await?;
            
            // Publish metadata event to feedback stream for background processing
            self.repository.publish_feedback_event(&json!({
                "event_type": "thought_created",
                "thought_id": thought_id,
                "instance": self.instance_id,
                "metadata": {
                    "importance": params.importance,
                    "relevance": params.relevance,
                    "tags": params.tags,
                    "category": params.category,
                },
                "timestamp": metadata.created_at,
            })).await?;
            
            tracing::info!("Saved metadata for thought {} with importance: {:?}, relevance: {:?}, tags: {:?}, category: {:?}", 
                thought_id, params.importance, params.relevance, params.tags, params.category);
        }
        
        // Display success and completion status
        self.visual.thought_stored(&thought_id);
        
        if !params.next_thought_needed {
            self.visual.thinking_complete();
        } else {
            self.visual.next_thought_indicator(true);
        }
        
        // Progress bar
        self.visual.progress_bar(params.thought_number, params.total_thoughts);
        
        Ok(ThinkResponse {
            status: "stored".to_string(),
            thought_id,
            next_thought_needed: params.next_thought_needed,
        })
    }
}