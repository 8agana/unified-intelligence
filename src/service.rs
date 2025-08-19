use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, Content, ErrorData, ServerCapabilities, ServerInfo},
};
use rmcp_macros::{tool, tool_handler, tool_router};
use std::future::Future;
use std::sync::Arc;

use crate::config::Config;
use crate::embeddings::generate_openai_embedding;
use crate::error::UnifiedIntelligenceError;
use crate::handlers::ToolHandlers;
use crate::handlers::help::{HelpHandlerTrait, UiHelpParams};
use crate::handlers::knowledge::KnowledgeHandler;
use crate::handlers::recall::UiRecallParams;
use crate::handlers::thoughts::ThoughtsHandler;
use crate::models::UiKnowledgeParams;
use crate::models::UiThinkParams;
use crate::rate_limit::RateLimiter;
use crate::redis::RedisManager;
use crate::repository::CombinedRedisRepository;
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};
use crate::synth::Synthesizer;
use crate::tools::ui_context::{UIContextParams, ui_context_impl};
use crate::tools::ui_memory::{UiMemoryParams, ui_memory_impl};
use crate::tools::ui_remember::{UiRememberParams, UiRememberResult};
use crate::tools::ui_start::{UiStartParams, UiStartResult};
use crate::validation::InputValidator;
use bytemuck::cast_slice;

/// Main service struct for UnifiedIntelligence MCP server
#[derive(Clone)]
pub struct UnifiedIntelligenceService {
    tool_router: ToolRouter<Self>,
    handlers: Arc<ToolHandlers<CombinedRedisRepository>>,
    rate_limiter: Arc<RateLimiter>,
    instance_id: String,
    config: Arc<Config>,
    // Qdrant removed; Redis is the sole storage backend
}

impl UnifiedIntelligenceService {
    /// Create a new service instance
    pub async fn new(redis_manager: Arc<RedisManager>) -> Result<Self, UnifiedIntelligenceError> {
        tracing::info!("Service::new() - Starting initialization");
        // Load configuration
        let config = Arc::new(Config::load());
        tracing::info!("Service::new() - Configuration loaded");

        // Get instance ID from environment or config
        let instance_id = std::env::var("INSTANCE_ID")
            .unwrap_or_else(|_| config.server.default_instance_id.clone());
        tracing::info!(
            "Service::new() - Initializing UnifiedIntelligence service for instance: {}",
            instance_id
        );

        // Initialize Bloom filter for this instance - DISABLED (requires RedisBloom)
        tracing::info!("Service::new() - Initializing Bloom filter (if enabled)");
        // redis_manager.init_bloom_filter(&instance_id).await?;
        tracing::info!("Service::new() - Bloom filter initialization skipped/completed");

        // Initialize event stream for this instance
        tracing::info!("Service::new() - Initializing event stream");
        redis_manager.init_event_stream(&instance_id).await?;
        tracing::info!("Service::new() - Event stream initialized");

        // Create repository with config and instance_id
        tracing::info!("Service::new() - Creating CombinedRedisRepository");
        let repository = Arc::new(CombinedRedisRepository::new(
            redis_manager.clone(),
            config.clone(),
            instance_id.clone(),
        ));
        tracing::info!("Service::new() - CombinedRedisRepository created");

        // Create validator
        tracing::info!("Service::new() - Creating InputValidator");
        let validator = Arc::new(InputValidator::new());
        tracing::info!("Service::new() - InputValidator created");

        // Create rate limiter with configured values
        tracing::info!("Service::new() - Creating RateLimiter");
        let rate_limiter = Arc::new(RateLimiter::new(
            config.rate_limiter.max_requests as usize,
            config.rate_limiter.window_seconds as u64,
        ));
        tracing::info!("Service::new() - RateLimiter created");

        // Create handlers
        tracing::info!("Service::new() - Creating ToolHandlers");
        let handlers = Arc::new(ToolHandlers::new(
            repository,
            instance_id.clone(),
            validator,
            redis_manager.clone(), // Pass redis_manager
        ));
        tracing::info!("Service::new() - ToolHandlers created");

        tracing::info!("Service::new() - Service initialization complete");
        Ok(Self {
            tool_router: Self::tool_router(),
            handlers,
            rate_limiter,
            instance_id,
            config,
        })
    }
}

#[tool_router]
impl UnifiedIntelligenceService {
    #[tool(description = "Capture and process thoughts with optional chaining support")]
    pub async fn ui_think(
        &self,
        params: Parameters<UiThinkParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        match self.handlers.ui_think(params.0).await {
            Ok(response) => {
                let content = Content::json(response).map_err(|e| {
                    ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => match &e {
                UnifiedIntelligenceError::DuplicateThought { .. } => {
                    tracing::warn!("Duplicate thought attempted: {}", e);
                    Err(ErrorData::invalid_params(e.to_string(), None))
                }
                _ => {
                    tracing::error!("ui_think error: {}", e);
                    Err(ErrorData::internal_error(e.to_string(), None))
                }
            },
        }
    }

    #[tool(description = "Retrieve thoughts and memories by ID or chain ID.")]
    pub async fn ui_recall(
        &self,
        params: Parameters<UiRecallParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        if params.0.mode == "help" {
            let help = serde_json::json!({
                "tool": "ui_recall",
                "usage": {
                    "mode": "thought|chain|help",
                    "id": "string (thought_id or chain_id)"
                },
                "examples": [
                    {"mode": "thought", "id": "<thought_id>"},
                    {"mode": "chain", "id": "<chain_id>"},
                    {"mode": "help", "id": "ignored"}
                ],
                "troubleshooting": [
                    "Ensure the ID exists in the current instance namespace",
                    "Use ui_help for a list of tools and high-level guidance"
                ]
            });
            let content = Content::json(help).map_err(|e| {
                ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
            })?;
            return Ok(CallToolResult::success(vec![content]));
        }

        match self.handlers.recall.recall(params.0).await {
            Ok(response) => {
                let content = Content::json(response).map_err(|e| {
                    ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                tracing::error!("ui_recall error: {}", e);
                Err(ErrorData::internal_error(
                    format!("Error recalling thought: {e}"),
                    None,
                ))
            }
        }
    }

    #[tool(description = "Get help information about available tools and their usage")]
    pub async fn ui_help(
        &self,
        params: Parameters<UiHelpParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // No rate limit for help requests
        match self.handlers.ui_help(params.0).await {
            Ok(response) => {
                let content = Content::json(response).map_err(|e| {
                    ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                tracing::error!("ui_help error: {}", e);
                Err(ErrorData::internal_error(
                    format!("Error generating help: {e}"),
                    None,
                ))
            }
        }
    }

    #[tool(description = "Manage knowledge graph entities and relationships")]
    pub async fn ui_knowledge(
        &self,
        params: Parameters<UiKnowledgeParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        if params.0.mode == "help" {
            let help = serde_json::json!({
                "tool": "ui_knowledge",
                "usage": {
                    "mode": "create|search|set_active|get_entity|create_relation|get_relations|update_entity|delete_entity|help",
                    "common": ["entity_id?", "scope?"],
                    "create/update": ["name?", "display_name?", "entity_type?", "attributes?", "tags?"],
                    "search": ["query?", "limit?"],
                    "relations": ["from_entity_id?", "to_entity_id?", "relationship_type?", "bidirectional?", "weight?"],
                },
                "troubleshooting": [
                    "Use scope Federation or Personal appropriately",
                    "Ensure entity names are unique within scope"
                ]
            });
            let content = Content::json(help).map_err(|e| {
                ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
            })?;
            return Ok(CallToolResult::success(vec![content]));
        }

        match self.handlers.ui_knowledge(params.0).await {
            Ok(response) => {
                let content = Content::json(response).map_err(|e| {
                    ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                tracing::error!("ui_knowledge error: {}", e);
                Err(ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(
        description = "Store UI context (personal|federation) with optional metadata or return help"
    )]
    pub async fn ui_context(
        &self,
        params: Parameters<UIContextParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Rate limit similar to other tools
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.",
                None,
            ));
        }

        // Standardized help for ui_context
        if params.0.kind.eq_ignore_ascii_case("help") {
            let help = serde_json::json!({
                "tool": "ui_context",
                "usage": {
                    "type": "personal|federation|help",
                    "content?": "string (required unless type=help)",
                    "category?": "string (e.g., session-summary, decision, insight)",
                    "tags?": "string[]",
                    "importance?": "string",
                    "chain_id?": "string",
                    "thought_id?": "string",
                    "instance_id?": "string"
                },
                "aliases": {
                    "personal": ["local", "instance", "private", "provide"],
                    "federation": ["federated", "team", "shared", "global"]
                },
                "examples": [
                    {"type": "provide", "content": "Session context", "category": "session-summary"},
                    {"type": "federation", "content": "Team decision", "category": "decision", "tags": ["architecture"]},
                    {"type": "help"}
                ],
                "troubleshooting": [
                    "Ensure RediSearch indices are created or allow tool to create them",
                    "OPENAI_API_KEY must be set for embeddings",
                    "INSTANCE_ID controls personal index namespace"
                ]
            });
            let content = Content::json(help).map_err(|e| {
                ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
            })?;
            return Ok(CallToolResult::success(vec![content]));
        }

        let result = ui_context_impl(&self.config, &self.handlers.redis_manager, params.0)
            .await
            .map_err(|e| {
                tracing::error!("ui_context error: {}", e);
                ErrorData::internal_error(e.to_string(), None)
            })?;

        let content = Content::json(result).map_err(|e| {
            ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![content]))
    }

    #[tool(description = "Search/read/update/delete memory across embeddings with simple filters")]
    pub async fn ui_memory(
        &self,
        params: Parameters<UiMemoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        // Standardized help for ui_memory
        if params.0.action.eq_ignore_ascii_case("help") {
            let help = serde_json::json!({
                "tool": "ui_memory",
                "usage": {
                    "action": "search|read|update|delete|help",
                    "query?": "string",
                    "scope?": "all|session-summaries|important|federation",
                    "filters?": {"tags?": "string[]", "importance?": "string", "chain_id?": "string", "thought_id?": "string"},
                    "options?": {"limit?": "number", "offset?": "number", "k?": "number", "search_type?": "string"},
                    "targets?": {"keys?": "string[]"},
                    "update?": {"content?": "string", "tags?": "string[]", "importance?": "string", "chain_id?": "string", "thought_id?": "string"}
                },
                "examples": [
                    {"action": "search", "query": "vector db", "scope": "all"},
                    {"action": "search", "query": "session summary", "scope": "session-summaries"},
                    {"action": "read", "targets": {"keys": ["CC:embeddings:important:abc123"]}},
                    {"action": "help"}
                ],
                "troubleshooting": [
                    "UTF-8 errors: avoids binary 'vector' field using HMGET for text fields",
                    "Empty results: confirm indices and scope",
                    "Set OPENAI_API_KEY for re-embedding on update"
                ]
            });
            let content = Content::json(help).map_err(|e| {
                ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
            })?;
            return Ok(CallToolResult::success(vec![content]));
        }

        match ui_memory_impl(&self.config, &self.handlers.redis_manager, params.0).await {
            Ok(response) => {
                let content = Content::json(response).map_err(|e| {
                    ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                tracing::error!("ui_memory error: {}", e);
                Err(ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(
        description = "Conversational memory: store user query, synthesize response, Redis-only"
    )]
    pub async fn ui_remember(
        &self,
        params: Parameters<UiRememberParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        // 0) Help mode
        let p = params.0;
        if matches!(p.action.as_deref(), Some("help")) {
            let help = serde_json::json!({
                "tool": "ui_remember",
                "usage": {
                    "action?": "help",
                    "thought": "string",
                    "thought_number": "integer",
                    "total_thoughts": "integer",
                    "chain_id?": "string",
                    "style?": "string (e.g., deep|chronological)",
                    "tags?": "string[]"
                },
                "flow": "T1 user thought -> T2 synthesized assistant -> T3 metrics and feedback hash",
                "troubleshooting": [
                    "Ensure RediSearch indices exist for hybrid retrieval",
                    "Set OPENAI_API_KEY and GROQ_API_KEY"
                ]
            });
            let content = Content::json(help).map_err(|e| {
                ErrorData::internal_error(format!("Failed to create JSON content: {e}"), None)
            })?;
            return Ok(CallToolResult::success(vec![content]));
        }

        // Normalize action: default to query; allow feedback aliases
        fn parse_action(action: &Option<String>, has_feedback: bool) -> &'static str {
            if has_feedback {
                return "feedback";
            }
            let a = action.as_deref().unwrap_or("query").to_ascii_lowercase();
            let norm: String = a.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            match norm.as_str() {
                "feedback" | "fb" | "critique" | "review" => "feedback",
                _ => "query",
            }
        }
        let action = parse_action(&p.action, p.feedback.is_some());

        // If this is an explicit feedback call, store Thought 3 (feedback) and return
        if action == "feedback" {
            let chain_id = p.chain_id.clone().ok_or_else(|| {
                ErrorData::invalid_params("chain_id is required for feedback".to_string(), None)
            })?;
            // Find latest assistant thought in the chain to attach feedback to
            let latest_assistant = self
                .handlers
                .repository
                .get_chain_thoughts(&self.instance_id, &chain_id)
                .await
                .ok()
                .and_then(|thoughts| {
                    thoughts
                        .into_iter()
                        .filter(|t| t.category.as_deref() == Some("ui_remember:assistant"))
                        .max_by_key(|t| t.timestamp.clone())
                });

            // Build feedback content
            let feedback_text = p.feedback.unwrap_or_default();
            let t3 = crate::models::ThoughtRecord::new(
                self.instance_id.clone(),
                feedback_text,
                p.thought_number.max(2) + 1, // place after T2
                p.total_thoughts.max(3),
                Some(chain_id.clone()),
                p.continue_next.unwrap_or(false),
                Some("ui_remember".to_string()),
                None,
                None,
                p.tags.clone(),
                Some("ui_remember:feedback".to_string()),
            );
            let thought3_id = t3.id.clone();
            if let Err(e) = self.handlers.repository.save_thought(&t3).await {
                tracing::error!("ui_remember: failed to save feedback T3: {}", e);
                return Err(ErrorData::internal_error(e.to_string(), None));
            }

            // Update feedback hash for latest assistant if available
            if let (Some(assistant), Ok(mut con)) = (
                latest_assistant,
                self.handlers.redis_manager.get_connection().await,
            ) {
                let key = format!("voice:feedback:{}", assistant.id);
                let _: () = redis::pipe()
                    .hset(&key, "llm_feedback", &t3.thought)
                    .hset(
                        &key,
                        "continue_next",
                        i32::from(p.continue_next.unwrap_or(false)),
                    )
                    .query_async(&mut *con)
                    .await
                    .unwrap_or(());
            }

            let result = UiRememberResult {
                status: "feedback_saved".to_string(),
                thought3_id: Some(thought3_id),
                next_action: if p.continue_next.unwrap_or(false) {
                    Some(crate::tools::ui_remember::NextAction {
                        tool: "ui_remember".to_string(),
                        action: "query".to_string(),
                        required: vec!["chain_id".to_string(), "thought".to_string()],
                        optional: vec!["style".to_string(), "tags".to_string()],
                    })
                } else {
                    None
                },
                ..Default::default()
            };
            let ack = Content::text(
                "Feedback stored. Set continue_next=true to proceed with another query.",
            );
            let content = Content::json(result)
                .map_err(|e| ErrorData::internal_error(format!("JSON encode error: {e}"), None))?;
            return Ok(CallToolResult::success(vec![ack, content]));
        }

        // 1) If there is a prior assistant synthesis in this chain, update its feedback from current user behavior
        if let Some(ref chain_id) = p.chain_id {
            if let Ok(chain_thoughts) = self
                .handlers
                .repository
                .get_chain_thoughts(&self.instance_id, chain_id)
                .await
            {
                if let Some(prev_assistant) = chain_thoughts
                    .iter()
                    .filter(|t| t.category.as_deref() == Some("ui_remember:assistant"))
                    .max_by_key(|t| t.timestamp.clone())
                {
                    let prev_ts = chrono::DateTime::parse_from_rfc3339(&prev_assistant.timestamp)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now());
                    let now = chrono::Utc::now();
                    let delta = (now - prev_ts).num_seconds().max(0);

                    let (score, abandoned, continued, corrected) =
                        compute_feedback_scoring(delta, &p.thought);

                    if let Ok(mut con) = self.handlers.redis_manager.get_connection().await {
                        let key = format!("voice:feedback:{}", prev_assistant.id);
                        let corrected_text = if corrected { &p.thought } else { "" };
                        let _: () = redis::pipe()
                            .hset(&key, "continued", continued)
                            .hset(&key, "time_to_next", delta)
                            .hset(&key, "corrected", corrected_text)
                            .hset(&key, "synthesis_quality", score)
                            .hset(&key, "feedback_score", score)
                            .hset(&key, "abandoned", abandoned)
                            .query_async(&mut *con)
                            .await
                            .unwrap_or(());
                    }
                }
            }
        }

        // 2) Store Thought 1 (user query)
        // Ensure chain_id exists; mint if missing; derive numbering from chain
        let chain_id = if let Some(c) = p.chain_id.clone() {
            c
        } else {
            format!("remember:{}", uuid::Uuid::new_v4())
        };
        let last_n = match self
            .handlers
            .repository
            .get_chain_thoughts(&self.instance_id, &chain_id)
            .await
        {
            Ok(v) => v.into_iter().map(|t| t.thought_number).max().unwrap_or(0),
            Err(_) => 0,
        };
        let t1 = crate::models::ThoughtRecord::new(
            self.instance_id.clone(),
            p.thought.clone(),
            last_n + 1,
            last_n + 2,
            Some(chain_id.clone()),
            true, // we know we'll produce at least one more thought
            Some("ui_remember".to_string()),
            None,
            None,
            p.tags.clone(),
            Some("ui_remember:user".to_string()),
        );
        let thought1_id = t1.id.clone();
        if let Err(e) = self.handlers.repository.save_thought(&t1).await {
            tracing::error!("ui_remember: failed to save T1: {}", e);
            return Err(ErrorData::internal_error(e.to_string(), None));
        }

        // 3) Retrieval: text search over thoughts + embedding KNN over memory indices
        let retrieved = match self
            .handlers
            .repository
            .search_thoughts(&self.instance_id, &p.thought, 0, 5)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "ui_remember: retrieval failed, continuing without context: {}",
                    e
                );
                Vec::new()
            }
        };

        // Embedding KNN across memory indexes
        // (key, optional_distance_score, content, ts)
        let mut knn_items: Vec<(String, Option<f64>, String, i64)> = Vec::new();
        if let Ok(openai_key) = self.config.openai.api_key() {
            if let Ok(embedding) =
                generate_openai_embedding(&p.thought, &openai_key, &self.handlers.redis_manager)
                    .await
            {
                let dims = self.config.openai.embedding_dimensions;
                if embedding.len() == dims {
                    let vec_bytes: Vec<u8> = cast_slice(&embedding).to_vec();
                    let instance_id = &self.instance_id;
                    let indexes = vec![
                        format!("idx:{instance_id}:session-summaries"),
                        format!("idx:{instance_id}:important"),
                        "idx:Federation:embeddings".to_string(),
                    ];
                    if let Ok(mut con) = self.handlers.redis_manager.get_connection().await {
                        for idx in indexes {
                            let val: redis::Value = redis::cmd("FT.SEARCH")
                                .arg(&idx)
                                .arg("*=>[KNN $k @vector $vec AS score]")
                                .arg("PARAMS")
                                .arg(4)
                                .arg("k")
                                .arg(5)
                                .arg("vec")
                                .arg(vec_bytes.as_slice())
                                .arg("SORTBY")
                                .arg("score")
                                .arg("LIMIT")
                                .arg(0)
                                .arg(5)
                                .arg("RETURN")
                                .arg(1)
                                .arg("score")
                                .arg("DIALECT")
                                .arg(2)
                                .query_async(&mut *con)
                                .await
                                .unwrap_or(redis::Value::Nil);
                            let keys_scores = extract_doc_ids_and_scores(&val);
                            if keys_scores.is_empty() {
                                continue;
                            }
                            // Use HMGET to fetch only text fields, avoiding binary vector field
                            let fields = ["content", "ts"];
                            let mut pipe = redis::pipe();
                            for (k, _) in &keys_scores {
                                pipe.cmd("HMGET").arg(k).arg(&fields);
                            }
                            let rows: Vec<Vec<Option<String>>> =
                                pipe.query_async(&mut *con).await.unwrap_or_default();
                            for (i, row) in rows.into_iter().enumerate() {
                                if i >= keys_scores.len() {
                                    break;
                                }
                                let mut it = row.into_iter();
                                let content = it.next().flatten().unwrap_or_default();
                                let ts = it
                                    .next()
                                    .and_then(|s| s.and_then(|x| x.parse::<i64>().ok()))
                                    .unwrap_or_default();
                                if !content.is_empty() {
                                    let (key, score_opt) = &keys_scores[i];
                                    knn_items.push((key.clone(), *score_opt, content, ts));
                                }
                            }
                        }
                    }
                }
            }
        } else {
            tracing::warn!(
                "OPENAI_API_KEY not available; skipping KNN vector search for ui_remember"
            );
        }
        let knn_count = knn_items.len();
        // Simple recency proxy: average age (seconds) of KNN items
        let now_ts = chrono::Utc::now().timestamp();
        let _avg_age_secs: i64 = if knn_count > 0 {
            let sum: i64 = knn_items
                .iter()
                .map(|(_, _, _, ts)| (now_ts - *ts).max(0))
                .sum();
            sum / (knn_count as i64)
        } else {
            0
        };

        // Build candidate set with simple hybrid scoring (semantic/text/recency)
        struct Cand {
            thought: crate::models::Thought,
            combined: f64,
        }
        let tau_secs: f64 = 86_400.0; // 1 day decay constant for recency
        let recency = |t: &chrono::DateTime<chrono::Utc>| -> f64 {
            let age = (chrono::Utc::now() - *t).num_seconds().max(0) as f64;
            (-age / tau_secs).exp()
        };

        let mut cands: Vec<Cand> = Vec::new();

        // Pull weights from config
        let w_sem = self.config.ui_remember.hybrid_weights.semantic;
        let w_text = self.config.ui_remember.hybrid_weights.text;
        let w_rec = self.config.ui_remember.hybrid_weights.recency;

        // Text hits -> text=1.0, semantic=0.0
        for r in &retrieved {
            let id = match uuid::Uuid::parse_str(&r.id) {
                Ok(u) => u,
                Err(_) => uuid::Uuid::new_v4(),
            };
            let ts = chrono::DateTime::parse_from_rfc3339(&r.timestamp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let text_score = 1.0f64;
            let semantic_score = 0.0f64;
            let rec = recency(&ts);
            let combined = w_sem * semantic_score + w_text * text_score + w_rec * rec;
            cands.push(Cand {
                thought: crate::models::Thought {
                    id,
                    content: r.content.clone(),
                    category: r.category.clone(),
                    tags: r.tags.clone().unwrap_or_default(),
                    instance_id: r.instance.clone(),
                    created_at: ts,
                    updated_at: ts,
                    importance: r.importance.unwrap_or(5),
                    relevance: r.relevance.unwrap_or(5),
                    semantic_score: None,
                    temporal_score: None,
                    usage_score: None,
                    combined_score: None,
                },
                combined,
            });
        }
        // KNN items -> semantic based on distance score, text=0.0
        for (_key, score_opt, content, ts) in &knn_items {
            let id = uuid::Uuid::new_v4();
            let tsdt = chrono::DateTime::from_timestamp(*ts, 0).unwrap_or_else(chrono::Utc::now);
            let text_score = 0.0f64;
            // Convert RediSearch vector score (distance; lower is better) to similarity in 0..1
            let semantic_score = score_opt.map(|d| 1.0f64 / (1.0f64 + d)).unwrap_or(0.5f64);
            let rec = recency(&tsdt);
            let combined = w_sem * semantic_score + w_text * text_score + w_rec * rec;
            cands.push(Cand {
                thought: crate::models::Thought {
                    id,
                    content: content.clone(),
                    category: None,
                    tags: vec![],
                    instance_id: self.instance_id.clone(),
                    created_at: tsdt,
                    updated_at: tsdt,
                    importance: 5,
                    relevance: 5,
                    semantic_score: None,
                    temporal_score: None,
                    usage_score: None,
                    combined_score: None,
                },
                combined,
            });
        }

        // Sort candidates and cap to top_k (default 5)
        let top_k_used: usize = p.top_k.map(|v| v as usize).unwrap_or(5);
        cands.sort_by(|a, b| {
            b.combined
                .partial_cmp(&a.combined)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let ctx_thoughts: Vec<crate::models::Thought> = cands
            .into_iter()
            .take(top_k_used)
            .map(|c| c.thought)
            .collect();

        // 3) Build intent and synthesize via Groq
        let intent = crate::models::QueryIntent {
            original_query: p.thought.clone(),
            temporal_filter: None,
            synthesis_style: p.style.clone(),
        };

        let groq_api_key = self.config.groq.api_key.clone();
        let tx = match crate::transport::GroqTransport::new(groq_api_key) {
            Ok(v) => std::sync::Arc::new(v) as std::sync::Arc<dyn crate::transport::Transport>,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };
        let synth = crate::synth::GroqSynth::new(
            tx,
            self.config.groq.model_fast.clone(),
            self.config.groq.model_deep.clone(),
        );

        let start = std::time::Instant::now();
        let synthesized = match synth.synth(&intent, &ctx_thoughts).await {
            Ok(s) => s,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };
        let _latency_ms = start.elapsed().as_millis() as i64;

        // 4) Store Thought 2 (assistant synthesis)
        let t2 = crate::models::ThoughtRecord::new(
            self.instance_id.clone(),
            synthesized.text.clone(),
            last_n + 2,
            last_n + 2,
            Some(chain_id.clone()),
            true, // a later metrics/feedback thought or next user thought
            Some("ui_remember".to_string()),
            None,
            None,
            p.tags.clone(),
            Some("ui_remember:assistant".to_string()),
        );
        let thought2_id = t2.id.clone();
        if let Err(e) = self.handlers.repository.save_thought(&t2).await {
            tracing::error!("ui_remember: failed to save T2: {}", e);
            return Err(ErrorData::internal_error(e.to_string(), None));
        }

        // 5) Prompt for LLM feedback (no metrics thought here). Seed feedback hash for T2.
        if let Ok(mut con) = self.handlers.redis_manager.get_connection().await {
            let key = format!("voice:feedback:{thought2_id}");
            let mut pipe = redis::pipe();
            pipe.hset(&key, "synthesis_quality", 0.0f32)
                .hset(&key, "continued", 0)
                .hset(&key, "abandoned", 0)
                .hset(&key, "corrected", "")
                .hset(&key, "time_to_next", -1)
                .hset(&key, "feedback_score", 0.0f32);
            let _: () = pipe.query_async(&mut *con).await.unwrap_or(());
        }

        let result = UiRememberResult {
            status: "ok".to_string(),
            thought1_id,
            thought2_id,
            thought3_id: None,
            model_used: Some(synthesized.model_used.clone()),
            usage_total_tokens: synthesized.usage.as_ref().and_then(|u| u.total_tokens),
            assistant_text: Some(synthesized.text.clone()),
            retrieved_text_count: Some(retrieved.len()),
            retrieved_embedding_count: Some(knn_count),
            next_action: Some(crate::tools::ui_remember::NextAction {
                tool: "ui_remember".to_string(),
                action: "feedback".to_string(),
                required: vec!["chain_id".to_string(), "feedback".to_string()],
                optional: vec!["continue_next".to_string()],
            }),
        };

        // Return assistant response and a feedback prompt
        let text_part = Content::text(synthesized.text.clone());
        let prompt = Content::text(
            "Provide feedback via ui_remember {action:\"feedback\", chain_id, feedback, continue_next?}.",
        );
        let json_part = Content::json(result)
            .map_err(|e| ErrorData::internal_error(format!("JSON encode error: {e}"), None))?;
        Ok(CallToolResult::success(vec![text_part, prompt, json_part]))
    }

    #[tool(description = "Start a session: summarize previous chain, embed, and set new chain_id")]
    pub async fn ui_start(
        &self,
        params: Parameters<UiStartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                "Rate limit exceeded. Please slow down your requests.".to_string(),
                None,
            ));
        }

        let p = params.0;
        let scope = p.scope.unwrap_or_default();

        // 1) Resolve user KG node
        let node = match self
            .handlers
            .repository
            .get_entity_by_name(&p.user, &scope)
            .await
        {
            Ok(n) => n,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };

        // Read previous session chain_id from attributes (if present)
        let prev_chain_id: Option<String> = node
            .attributes
            .get("current_session_chain_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 2) Retrieve previous session thoughts (if any)
        let prev_thoughts = if let Some(ref chain_id) = prev_chain_id {
            match self
                .handlers
                .repository
                .get_chain_thoughts(&self.instance_id, chain_id)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("ui_start: failed to retrieve chain {}: {}", chain_id, e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // 3) Summarize previous session into structured 10k summary
        // Build a prompt using your structure and budget tokens conservatively
        let summary_schema = r#"
You are an assistant producing a structured session summary for LLM consumption.
Return only the summary text, following exactly these sections and headings:
1) Critical Status & Warnings
2) Identity & Relationship Dynamics (includes running jokes)
3) Session Narrative & Context (includes new jokes)
4) Technical Work In Progress
5) Technical Work Completed
6) System Relationships & Architecture
7) Decisions Made & Rationale
8) Active Conversations & Threads
9) Lessons Learned & Insights
10) Next Actions & Continuation Points

Guidelines:
- Be faithful to source thoughts; avoid speculation.
- Keep it self-contained and readable.
- Target ~10,000 tokens max; compress thoughtfully if needed.
"#;

        // Convert ThoughtRecord -> simple Thought for synth context
        let ctx_thoughts: Vec<crate::models::Thought> = prev_thoughts
            .iter()
            .map(|t| crate::models::Thought {
                id: uuid::Uuid::new_v4(),
                content: t.thought.clone(),
                category: t.category.clone(),
                tags: t.tags.clone().unwrap_or_default(),
                instance_id: self.instance_id.clone(),
                created_at: chrono::DateTime::parse_from_rfc3339(&t.timestamp)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: chrono::Utc::now(),
                importance: t.importance.unwrap_or(5),
                relevance: t.relevance.unwrap_or(5),
                semantic_score: None,
                temporal_score: None,
                usage_score: None,
                combined_score: None,
            })
            .collect();

        let groq_api_key = self.config.groq.api_key.clone();
        let tx = match crate::transport::GroqTransport::new(groq_api_key) {
            Ok(v) => std::sync::Arc::new(v) as std::sync::Arc<dyn crate::transport::Transport>,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };
        let synth = crate::synth::GroqSynth::new(
            tx,
            self.config.groq.model_fast.clone(),
            self.config.groq.model_deep.clone(),
        );

        let intent = crate::models::QueryIntent {
            original_query: format!(
                "Produce a structured session summary following the given headings and guidance.\n\n{summary_schema}"
            ),
            temporal_filter: None,
            synthesis_style: Some("deep".to_string()),
        };

        let synthesized = match synth.synth(&intent, &ctx_thoughts).await {
            Ok(s) => s,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };
        let summary_text = synthesized.text.clone();

        // 4) Store summary JSON (no TTL); embed in chunks as HASH docs with no TTL
        let summary_key = format!(
            "{}:ui_start:summary:{}",
            self.instance_id,
            prev_chain_id.clone().unwrap_or_else(|| "none".to_string())
        );
        if let Err(e) = self
            .handlers
            .redis_manager
            .json_set(
                &summary_key,
                "$",
                &serde_json::json!({
                    "chain_id": prev_chain_id,
                    "summary": summary_text,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "model_used": synthesized.model_used,
                }),
            )
            .await
        {
            tracing::error!("ui_start: failed to store summary JSON: {}", e);
        }
        // No TTL set on summary JSON per current policy

        // Ensure RediSearch index for session summaries exists
        let dims = self.config.openai.embedding_dimensions;
        let index = format!("idx:{}:session-summaries", self.instance_id);
        let prefix = format!("{}:embeddings:session-summaries:", self.instance_id);
        let _ = ensure_index_hash_hnsw(
            &self.handlers.redis_manager,
            &index,
            &prefix,
            dims,
            self.config.redis_search.hnsw.m,
            self.config.redis_search.hnsw.ef_construction,
        )
        .await;

        // Embed summary in chunks (~2000 chars per chunk) and store as HASH docs (persistent)
        let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        if !openai_api_key.is_empty() {
            let mut start = 0usize;
            let chunk_size = 2000usize;
            let bytes = summary_text.as_bytes();
            while start < bytes.len() {
                let end = (start + chunk_size).min(bytes.len());
                let chunk = &summary_text[start..end];
                if let Ok(embedding) =
                    generate_openai_embedding(chunk, &openai_api_key, &self.handlers.redis_manager)
                        .await
                {
                    if let Ok(mut con) = self.handlers.redis_manager.get_connection().await {
                        let key = format!(
                            "{}:embeddings:session-summaries:{}:{}",
                            self.instance_id,
                            prev_chain_id.clone().unwrap_or_else(|| "none".to_string()),
                            start
                        );
                        let vec_bytes: Vec<u8> = cast_slice(&embedding).to_vec();
                        let ts = chrono::Utc::now().timestamp();
                        let tags_csv = "session-summary";
                        let _: () = redis::pipe()
                            .hset(&key, "content", chunk)
                            .hset(&key, "tags", tags_csv)
                            .hset(&key, "category", "session-summary")
                            .hset(&key, "importance", "")
                            .hset(
                                &key,
                                "chain_id",
                                prev_chain_id.clone().unwrap_or_else(|| "none".to_string()),
                            )
                            .hset(&key, "thought_id", "")
                            .hset(&key, "ts", ts)
                            .hset(&key, "vector", vec_bytes)
                            .query_async(&mut *con)
                            .await
                            .unwrap_or(());
                    }
                }
                start = end;
            }
        } else {
            tracing::warn!("OPENAI_API_KEY not set; skipping embeddings in ui_start");
        }

        // 5) Create new chain_id for current session and update KG
        let new_chain_id = format!("session:{}", uuid::Uuid::new_v4());
        let mut updated = node.clone();
        let mut attrs = updated.attributes.clone();
        // Maintain session_history as array of chain_ids
        let mut history: Vec<String> = attrs
            .get("session_history")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(prev) = prev_chain_id.clone() {
            if !prev.is_empty() {
                history.insert(0, prev);
            }
        }
        attrs.insert(
            "session_history".to_string(),
            serde_json::Value::Array(history.into_iter().map(serde_json::Value::String).collect()),
        );
        attrs.insert(
            "current_session_chain_id".to_string(),
            serde_json::Value::String(new_chain_id.clone()),
        );
        // Track summary key by previous chain_id
        let mut summary_map = attrs
            .get("summary_keys")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        summary_map.insert(
            prev_chain_id.clone().unwrap_or_else(|| "none".to_string()),
            serde_json::Value::String(summary_key.clone()),
        );
        attrs.insert(
            "summary_keys".to_string(),
            serde_json::Value::Object(summary_map),
        );
        updated.attributes = attrs;
        if let Err(e) = self
            .handlers
            .repository
            .update_entity(updated.clone())
            .await
        {
            tracing::error!("ui_start: failed to update KG entity: {}", e);
        }

        // 6) Return structured result with the summary text (MVP)
        let result = UiStartResult {
            status: "ok".to_string(),
            new_chain_id,
            summary_key,
            summary_text,
            model_used: Some(synthesized.model_used),
            usage_total_tokens: synthesized.usage.and_then(|u| u.total_tokens),
        };
        let content = Content::json(result)
            .map_err(|e| ErrorData::internal_error(format!("JSON encode error: {e}"), None))?;
        Ok(CallToolResult::success(vec![content]))
    }
}

// Ensure an HNSW RediSearch index exists for HASH prefixes
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
        .arg(10)
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

    match create_res {
        Ok(()) => Ok(true),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("index already exists") {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

// Extract doc ids and optional score from FT.SEARCH responses
fn extract_doc_ids_and_scores(val: &redis::Value) -> Vec<(String, Option<f64>)> {
    let mut out = Vec::new();
    match val {
        redis::Value::Array(items) if !items.is_empty() => {
            let mut i = 1usize; // skip total count
            while i < items.len() {
                // Expect id first
                let id_opt = match &items[i] {
                    redis::Value::BulkString(bytes) => {
                        std::str::from_utf8(bytes).ok().map(|s| s.to_string())
                    }
                    redis::Value::SimpleString(s) => Some(s.clone()),
                    _ => None,
                };
                i += 1;
                if id_opt.is_none() {
                    continue;
                }
                let mut score_opt: Option<f64> = None;
                // Next may be an array of field-value pairs
                if i < items.len() {
                    if let redis::Value::Array(fields) = &items[i] {
                        let mut j = 0usize;
                        while j + 1 < fields.len() {
                            let k = &fields[j];
                            let v = &fields[j + 1];
                            let key_is_score = match k {
                                redis::Value::BulkString(b) => std::str::from_utf8(b)
                                    .ok()
                                    .map(|s| s == "score")
                                    .unwrap_or(false),
                                redis::Value::SimpleString(s) => s == "score",
                                _ => false,
                            };
                            if key_is_score {
                                if let Some(num) = match v {
                                    redis::Value::BulkString(b) => std::str::from_utf8(b)
                                        .ok()
                                        .and_then(|s| s.parse::<f64>().ok()),
                                    redis::Value::SimpleString(s) => s.parse::<f64>().ok(),
                                    redis::Value::Int(i) => Some(*i as f64),
                                    _ => None,
                                } {
                                    score_opt = Some(num);
                                    break;
                                }
                            }
                            j += 2;
                        }
                        i += 1;
                    }
                }
                if let Some(id) = id_opt {
                    out.push((id, score_opt));
                }
            }
        }
        _ => {}
    }
    out
}

// Helper for computing feedback metrics heuristics
fn compute_feedback_scoring(delta_secs: i64, user_text: &str) -> (f64, i32, i32, bool) {
    let lc = user_text.to_lowercase();
    let corrected = [
        "actually",
        "no,",
        "that's not",
        "incorrect",
        "correction",
        "wrong",
        "not true",
        "should be",
    ]
    .iter()
    .any(|s| lc.contains(s));

    let positive_ack = [
        "thanks",
        "thank you",
        "got it",
        "great",
        "perfect",
        "that works",
        "awesome",
        "nice",
    ]
    .iter()
    .any(|s| lc.contains(s));

    let mut score = if corrected {
        0.3
    } else if delta_secs < 30 {
        0.9
    } else if delta_secs < 120 {
        0.7
    } else {
        0.5
    };

    if positive_ack {
        score = (score + 0.1f64).min(1.0f64);
    }
    if corrected {
        score = (score - 0.1f64).max(0.0f64);
    }

    let abandoned = if delta_secs >= 600 { 1 } else { 0 };
    let continued = 1; // this helper is used only when we see a follow-up
    (score, abandoned, continued, corrected)
}

#[tool_handler]
impl ServerHandler for UnifiedIntelligenceService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::V_2024_11_05,
            server_info: rmcp::model::Implementation {
                name: self.config.server.name.clone(),
                version: self.config.server.version.clone(),
            },
            capabilities: ServerCapabilities {
                tools: Some(Default::default()),
                ..Default::default()
            },
            instructions: Some(
                "UnifiedIntelligence MCP Server for Redis-backed thought storage".into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_doc_ids_and_scores_with_scores() {
        let val = redis::Value::Array(vec![
            redis::Value::Int(2),
            redis::Value::BulkString(b"doc:1".to_vec()),
            redis::Value::Array(vec![
                redis::Value::BulkString(b"score".to_vec()),
                redis::Value::BulkString(b"0.123".to_vec()),
            ]),
            redis::Value::BulkString(b"doc:2".to_vec()),
            redis::Value::Array(vec![
                redis::Value::BulkString(b"score".to_vec()),
                redis::Value::BulkString(b"0.456".to_vec()),
            ]),
        ]);

        let out = extract_doc_ids_and_scores(&val);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "doc:1");
        assert!((out[0].1.unwrap() - 0.123).abs() < 1e-9);
        assert_eq!(out[1].0, "doc:2");
        assert!((out[1].1.unwrap() - 0.456).abs() < 1e-9);
    }

    #[test]
    fn test_extract_doc_ids_and_scores_nocontent() {
        let val = redis::Value::Array(vec![
            redis::Value::Int(2),
            redis::Value::BulkString(b"doc:1".to_vec()),
            redis::Value::BulkString(b"doc:2".to_vec()),
        ]);
        let out = extract_doc_ids_and_scores(&val);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "doc:1");
        assert!(out[0].1.is_none());
        assert_eq!(out[1].0, "doc:2");
        assert!(out[1].1.is_none());
    }

    #[test]
    fn test_compute_feedback_scoring_thresholds() {
        // Fast continuation, positive ack
        let (s1, a1, c1, corr1) = compute_feedback_scoring(10, "Thanks, that works");
        assert!(s1 >= 0.9 && s1 <= 1.0);
        assert_eq!(a1, 0);
        assert_eq!(c1, 1);
        assert!(!corr1);

        // Normal continuation
        let (s2, a2, c2, corr2) = compute_feedback_scoring(60, "okay");
        assert!((s2 - 0.7).abs() < 1e-9);
        assert_eq!(a2, 0);
        assert_eq!(c2, 1);
        assert!(!corr2);

        // Slow continuation
        let (s3, a3, c3, corr3) = compute_feedback_scoring(180, "following up");
        assert!((s3 - 0.5).abs() < 1e-9);
        assert_eq!(a3, 0);
        assert_eq!(c3, 1);
        assert!(!corr3);

        // Correction case
        let (s4, a4, c4, corr4) = compute_feedback_scoring(20, "Actually, that's not correct");
        assert!(s4 <= 0.3);
        assert_eq!(a4, 0);
        assert_eq!(c4, 1);
        assert!(corr4);

        // Abandoned threshold (helper always marks continued=1; abandon detection handled elsewhere by elapsed time)
        let (s5, a5, _c5, _corr5) = compute_feedback_scoring(1200, "");
        assert_eq!(a5, 1);
        assert!(s5 <= 0.5);
    }
}
