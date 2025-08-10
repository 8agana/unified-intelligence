use crate::embeddings::generate_openai_embedding;
use crate::error::{Result, UnifiedIntelligenceError};
use crate::frameworks::{FrameworkProcessor, FrameworkVisual, ThinkingFramework};
use crate::intent::{GroqIntent, IntentParser};
use crate::models::{ChainMetadata, ThinkResponse, ThoughtRecord, UiThinkParams};
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};
use crate::synth::{GroqSynth, Synthesizer};
use crate::transport::{GroqTransport, Transport};
use std::env;
use std::sync::Arc;

/// Trait for thought-related operations
pub trait ThoughtsHandler {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse>;
}

impl<R: ThoughtRepository + KnowledgeRepository> ThoughtsHandler for super::ToolHandlers<R> {
    /// Handle ui_think tool
    async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse> {
        // Determine framework with graceful fallback to Socratic on invalid
        let framework = if let Some(ref framework_str) = params.framework {
            // Use safe parsing that defaults to Socratic on invalid input
            let parsed = ThinkingFramework::from_string_safe(framework_str);

            // Log a warning if the framework was invalid but continue with default
            if ThinkingFramework::from_string(framework_str).is_err() {
                tracing::warn!(
                    "Invalid framework '{}' provided, defaulting to Socratic",
                    framework_str
                );
                // Note: Using info log instead of visual warning since visual doesn't have warning method
                tracing::info!(
                    "Framework '{}' not recognized, using Socratic default",
                    framework_str
                );
            }

            parsed
        } else {
            ThinkingFramework::Socratic
        };

        // Display visual start with framework
        self.visual
            .thought_start(params.thought_number, params.total_thoughts);
        FrameworkVisual::display_framework_start(&framework);
        self.visual.thought_content(&params.thought);

        // Process through framework
        if framework != ThinkingFramework::Socratic {
            let processor = FrameworkProcessor::new(framework.clone());
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
            Some(framework.to_string()),
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

        let mut auto_generated_thought: Option<ThoughtRecord> = None;

        // Handle Remember and DeepRemember frameworks - automatically create thought 2 with search results
        if (framework == ThinkingFramework::Remember
            || framework == ThinkingFramework::DeepRemember)
            && params.thought_number == 1
        {
            let is_deep = framework == ThinkingFramework::DeepRemember;
            tracing::info!(
                "{} framework detected, initiating RAG search and synthesis",
                if is_deep { "DeepRemember" } else { "Remember" }
            );

            let openai_api_key = env::var("OPENAI_API_KEY").map_err(|_| {
                UnifiedIntelligenceError::EnvVar("OPENAI_API_KEY not found".to_string())
            })?;
            let groq_api_key_val = self.config.groq.api_key.clone();
            let groq_transport = Arc::new(GroqTransport::new(groq_api_key_val.clone())?);
            let groq_intent_parser = GroqIntent::new(
                Arc::clone(&groq_transport) as Arc<dyn Transport>,
                self.config.groq.intent_model.clone(),
            );
            let groq_synth = GroqSynth::new(
                Arc::clone(&groq_transport) as Arc<dyn Transport>,
                self.config.groq.model_fast.clone(),
                self.config.groq.model_deep.clone(),
            );

            // 1. Parse query intent using Groq
            let query_intent = match groq_intent_parser.parse(&params.thought).await {
                Ok(intent) => {
                    tracing::info!(
                        "Parsed query intent: original_query='{}', temporal_filter={:?}, synthesis_style={:?}",
                        intent.original_query,
                        intent.temporal_filter,
                        intent.synthesis_style
                    );
                    intent
                }
                Err(e) => {
                    tracing::error!("Failed to parse query intent with Groq: {}", e);
                    self.visual
                        .error(&format!("Remember framework intent parsing error: {e}"));
                    // Fallback to original thought and default synthesis style
                    crate::models::QueryIntent {
                        original_query: params.thought.clone(),
                        temporal_filter: None,
                        synthesis_style: None,
                    }
                }
            };

            // Use the original_query from the parsed intent for embedding
            let query_for_embedding = query_intent.original_query.clone();

            // 2. Generate embedding for the original query content
            match generate_openai_embedding(
                &query_for_embedding,
                &openai_api_key,
                &self.redis_manager,
            )
            .await
            {
                Ok(query_embedding) => {
                    tracing::info!("Generated embedding for query: {}.", query_for_embedding);

                    // 3. Query Qdrant vector database for similar vectors, applying temporal filter if present
                    let similarity_threshold = env::var("QDRANT_SIMILARITY_THRESHOLD")
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.35);

                    tracing::info!(
                        "Using similarity threshold: {} for remember framework",
                        similarity_threshold
                    );

                    match self
                        .qdrant_service
                        .search_memories(
                            query_embedding,
                            5, // top_k
                            Some(similarity_threshold),
                            None, // No temporal filter from parse_search_query
                        )
                        .await
                    {
                        Ok(retrieved_memories_val) => {
                            tracing::info!(
                                "Retrieved {} memories from Qdrant.",
                                retrieved_memories_val.len()
                            );

                            if retrieved_memories_val.is_empty() {
                                tracing::warn!("No relevant memories found in Qdrant.");
                                let thought2_content = format!(
                                    "No relevant memories found for: {query_for_embedding}"
                                );
                                let thought2 = ThoughtRecord::new(
                                    self.instance_id.clone(),
                                    thought2_content,
                                    2,
                                    params.total_thoughts.max(2),
                                    params.chain_id.clone(),
                                    params.total_thoughts > 2,
                                    Some(framework.to_string()),
                                    None,
                                    None,
                                    None,
                                    None,
                                );
                                self.repository.save_thought(&thought2).await?;
                                auto_generated_thought = Some(thought2);
                            } else {
                                // 4. Synthesize response using Groq, applying synthesis style if present
                                match groq_synth
                                    .synth(&query_intent, retrieved_memories_val.as_slice())
                                    .await
                                {
                                    Ok(synthesized_response) => {
                                        tracing::info!("Groq synthesis completed successfully.");

                                        let thought2_content = synthesized_response;

                                        let thought2 = ThoughtRecord::new(
                                            self.instance_id.clone(),
                                            thought2_content,
                                            2,                            // Always thought number 2
                                            params.total_thoughts.max(2), // Ensure at least 2 thoughts
                                            params.chain_id.clone(),
                                            params.total_thoughts > 2, // Continue if more than 2 thoughts expected
                                            Some(framework.to_string()),
                                            None,
                                            None,
                                            None,
                                            None,
                                        );

                                        let thought2_id = thought2.id.clone();

                                        // Save thought 2
                                        self.repository.save_thought(&thought2).await?;

                                        // Visual indicators for thought 2
                                        self.visual.thought_start(2, params.total_thoughts.max(2));
                                        self.visual.chain_info(
                                            &format!(
                                                "[Auto-generated from {} framework]",
                                                if is_deep { "DeepRemember" } else { "Remember" }
                                            ),
                                            false,
                                        );
                                        self.visual.thought_stored(&thought2_id);

                                        tracing::info!(
                                            "Created automatic thought 2 with ID: {}",
                                            thought2_id
                                        );
                                        auto_generated_thought = Some(thought2);
                                    }
                                    Err(e) => {
                                        tracing::error!("Groq synthesis failed: {}", e);
                                        self.visual.error(&format!(
                                            "Remember framework synthesis error: {e}"
                                        ));
                                        // Continue without failing the entire operation
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Qdrant search failed: {}", e);
                            self.visual
                                .error(&format!("Remember framework Qdrant error: {e}"));
                            // Continue without failing the entire operation
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("OpenAI embedding failed: {}", e);
                    self.visual
                        .error(&format!("Remember framework embedding error: {e}"));
                    // Continue without failing the entire operation
                }
            }
        }

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
