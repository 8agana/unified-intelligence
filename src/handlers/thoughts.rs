use crate::error::Result;
use crate::frameworks::{FrameworkProcessor, FrameworkVisual, ThinkingMode, WorkflowState};
use crate::models::{ChainMetadata, ThinkResponse, ThoughtRecord, UiThinkParams};
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};

/// Trait for thought-related operations
pub trait ThoughtsHandler {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse>;
}

impl<R: ThoughtRepository + KnowledgeRepository> ThoughtsHandler for super::ToolHandlers<R> {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse> {
        // Use parsed, forgiving framework_state (defaults to Conversation)
        let state: WorkflowState = params.framework_state;

        // Show framework banner and choose a thinking mode based on state (first recommended), if any
        self.visual.framework_state(state);
        // Choose a thinking mode based on state (first recommended), if any
        let chosen_mode: Option<ThinkingMode> = state.thinking_modes().first().copied();

        // Display visual start with framework
        self.visual
            .thought_start(params.thought_number, params.total_thoughts);
        if let Some(mode) = chosen_mode {
            FrameworkVisual::display_framework_start(&mode);
        }
        self.visual.thought_content(&params.thought);

        // Process through framework
        if let Some(mode) = chosen_mode {
            let processor = FrameworkProcessor::new(mode);
            let result = processor.process_thought(&params.thought, params.thought_number);
            FrameworkVisual::display_insights(&result.insights);
            FrameworkVisual::display_prompts(&result.prompts);
        }

        // Validate input
        self.validator.validate_thought_content(&params.thought)?;
        self.validator
            .validate_thought_numbers(params.thought_number, params.total_thoughts)?;
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
            params.thought.clone(),
            params.thought_number,
            params.total_thoughts,
            params.chain_id.clone(),
            params.next_thought_needed,
            Some(state.to_string()),
            params.importance,
            params.relevance,
            params.tags.clone(),
            params.category.clone(),
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

        let auto_generated_thought: Option<ThoughtRecord> = None;

        // Display success and completion status
        self.visual.thought_stored(&thought_id);

        if !params.next_thought_needed {
            self.visual.thinking_complete();
        } else {
            self.visual.next_thought_indicator(true);
        }

        // Progress bar
        self.visual
            .progress_bar(params.thought_number, params.total_thoughts);

        Ok(ThinkResponse {
            status: "stored".to_string(),
            thought_id,
            next_thought_needed: params.next_thought_needed,
            auto_generated_thought,
        })
    }
}
