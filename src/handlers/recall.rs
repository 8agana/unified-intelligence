use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};
use rmcp::model::{CallToolResult, Content, ErrorData};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UiRecallParams {
    #[schemars(regex(pattern = r"^(thought|chain)$"))]
    pub mode: String,
    pub id: String,
}

pub struct RecallHandler<R: ThoughtRepository> {
    repository: Arc<R>,
    instance_id: String,
}

impl<R: ThoughtRepository + KnowledgeRepository> RecallHandler<R> {
    pub fn new(repository: Arc<R>, instance_id: String) -> Self {
        Self {
            repository,
            instance_id,
        }
    }

    pub async fn recall(
        &self,
        params: UiRecallParams,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Validate mode string (regex validation should already handle this at deserialization)
        match params.mode.as_str() {
            "thought" => {
                let thought_id = params.id;

                match self
                    .repository
                    .get_thought(&self.instance_id, &thought_id)
                    .await
                {
                    Ok(Some(thought)) => {
                        info!("Successfully recalled thought: {}", thought_id);
                        let content = Content::json(thought).map_err(|e| {
                            ErrorData::internal_error(
                                format!("Failed to serialize thought: {e}"),
                                None,
                            )
                        })?;
                        Ok(CallToolResult::success(vec![content]))
                    }
                    Ok(None) => {
                        warn!("Thought not found: {}", thought_id);
                        Err(ErrorData::invalid_params(
                            format!("Thought with ID {thought_id} not found."),
                            None,
                        ))
                    }
                    Err(e) => {
                        warn!("Error recalling thought {}: {}", thought_id, e);
                        Err(ErrorData::internal_error(
                            format!("Error recalling thought: {e}"),
                            None,
                        ))
                    }
                }
            }
            "chain" => {
                let chain_id = params.id;

                match self
                    .repository
                    .get_chain_thoughts(&self.instance_id, &chain_id)
                    .await
                {
                    Ok(thoughts) => {
                        info!(
                            "Successfully recalled chain {}: {} thoughts",
                            chain_id,
                            thoughts.len()
                        );
                        let content = Content::json(thoughts).map_err(|e| {
                            ErrorData::internal_error(
                                format!("Failed to serialize chain thoughts: {e}"),
                                None,
                            )
                        })?;
                        Ok(CallToolResult::success(vec![content]))
                    }
                    Err(e) => {
                        warn!("Error recalling chain {}: {}", chain_id, e);
                        Err(ErrorData::internal_error(
                            format!("Error recalling chain: {e}"),
                            None,
                        ))
                    }
                }
            }
            _ => {
                // This should never happen due to regex validation, but we handle it gracefully
                warn!("Invalid recall mode: {}", params.mode);
                Err(ErrorData::invalid_params(
                    format!(
                        "Invalid recall mode '{}'. Must be 'thought' or 'chain'.",
                        params.mode
                    ),
                    None,
                ))
            }
        }
    }
}
