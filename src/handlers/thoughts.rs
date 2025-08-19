use crate::config::Config;
use crate::embeddings::generate_openai_embedding;
use crate::error::Result;
use crate::frameworks::{
    FrameworkProcessor, FrameworkVisual, StuckTracker, ThinkingMode, WorkflowState,
};
use crate::models::{ChainMetadata, ThinkResponse, ThoughtRecord, UiThinkParams};
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};
use bytemuck::cast_slice;

/// Trait for thought-related operations
pub trait ThoughtsHandler {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse>;
}

// Local helper to ensure an HNSW RediSearch index exists for HASH prefixes
async fn ensure_index_hash_hnsw(
    redis_manager: &crate::redis::RedisManager,
    index: &str,
    prefix: &str,
    dims: usize,
    m: u32,
    ef_construction: u32,
) -> std::result::Result<bool, redis::RedisError> {
    let mut con = redis_manager.get_connection().await.map_err(|e| match e {
        crate::error::UnifiedIntelligenceError::Redis(e) => e,
        other => redis::RedisError::from(std::io::Error::other(other.to_string())),
    })?;

    let info: redis::RedisResult<redis::Value> = redis::cmd("FT.INFO")
        .arg(index)
        .query_async(&mut *con)
        .await;
    if info.is_ok() {
        return Ok(false);
    }

    let create_res: redis::RedisResult<()> = redis::cmd("FT.CREATE")
        .arg(index)
        .arg("ON")
        .arg("HASH")
        .arg("PREFIX")
        .arg(1)
        .arg(prefix)
        .arg("SCHEMA")
        .arg("content")
        .arg("TEXT")
        .arg("tags")
        .arg("TAG")
        .arg("SEPARATOR")
        .arg(",")
        .arg("category")
        .arg("TEXT")
        .arg("importance")
        .arg("TEXT")
        .arg("chain_id")
        .arg("TEXT")
        .arg("thought_id")
        .arg("TEXT")
        .arg("ts")
        .arg("NUMERIC")
        .arg("SORTABLE")
        .arg("vector")
        .arg("VECTOR")
        .arg("HNSW")
        .arg("6")
        .arg("TYPE")
        .arg("FLOAT32")
        .arg("DIM")
        .arg(dims)
        .arg("DISTANCE_METRIC")
        .arg("COSINE")
        .arg("M")
        .arg(m)
        .arg("EF_CONSTRUCTION")
        .arg(ef_construction)
        .query_async(&mut *con)
        .await;

    create_res.map(|_| true)
}

impl<R: ThoughtRepository + KnowledgeRepository> ThoughtsHandler for super::ToolHandlers<R> {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse> {
        // Use parsed, forgiving framework_state (defaults to Conversation)
        let state: WorkflowState = params.framework_state;

        // Show framework banner and choose a thinking mode (persisting cycle if stuck)
        self.visual.framework_state(state);
        let chosen_mode: Option<ThinkingMode> = if matches!(state, WorkflowState::Stuck) {
            if let Some(ref chain_id) = params.chain_id {
                // Persist StuckTracker per chain: {instance}:stuck:chain:{chain_id}
                let key = format!("{}:stuck:chain:{}", self.instance_id, chain_id);
                // Load or initialize tracker
                let mut tracker: StuckTracker = match self
                    .redis_manager
                    .json_get::<StuckTracker>(&key, "$")
                    .await
                    .ok()
                    .flatten()
                {
                    Some(t) => t,
                    None => StuckTracker::new(chain_id.clone()),
                };
                // Pick next approach and persist
                let next = tracker.next_approach();
                let _ = self.redis_manager.json_set(&key, "$", &tracker).await;
                Some(next)
            } else {
                // Fallback to first recommended if no chain
                state.thinking_modes().first().copied()
            }
        } else {
            // Non-stuck: use the first recommended mode, if any
            state.thinking_modes().first().copied()
        };

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

        // Embed-on-save (best-effort, non-fatal)
        if let Ok(openai_key) = std::env::var("OPENAI_API_KEY") {
            if !openai_key.is_empty() {
                // Ensure index exists for thoughts embeddings
                let config = Config::load();
                let dims = config.openai.embedding_dimensions;
                let index = format!("idx:{}:thought", self.instance_id);
                let prefix = format!("{}:embeddings:thought:", self.instance_id);
                let _ = ensure_index_hash_hnsw(
                    &self.redis_manager,
                    &index,
                    &prefix,
                    dims,
                    config.redis_search.hnsw.m,
                    config.redis_search.hnsw.ef_construction,
                )
                .await;

                if let Ok(embedding) =
                    generate_openai_embedding(&params.thought, &openai_key, &self.redis_manager)
                        .await
                {
                    if embedding.len() == dims {
                        if let Ok(mut con) = self.redis_manager.get_connection().await {
                            let key =
                                format!("{}:embeddings:thought:{}", self.instance_id, thought.id);
                            let vec_bytes: Vec<u8> = cast_slice(&embedding).to_vec();
                            let ts = chrono::Utc::now().timestamp();
                            let tags_csv = params
                                .tags
                                .as_ref()
                                .map(|v| v.join(","))
                                .unwrap_or_default();
                            let _: () = redis::pipe()
                                .hset(&key, "content", &params.thought)
                                .hset(&key, "tags", tags_csv)
                                .hset(
                                    &key,
                                    "category",
                                    params.category.clone().unwrap_or_default(),
                                )
                                .hset(
                                    &key,
                                    "importance",
                                    params.importance.unwrap_or(5).to_string(),
                                )
                                .hset(
                                    &key,
                                    "chain_id",
                                    params.chain_id.clone().unwrap_or_default(),
                                )
                                .hset(&key, "thought_id", &thought.id)
                                .hset(&key, "ts", ts)
                                .hset(&key, "vector", vec_bytes)
                                .query_async(&mut *con)
                                .await
                                .unwrap_or(());
                        }
                    }
                }
            }
        }

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
