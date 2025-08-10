use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, Content, ErrorData, ServerCapabilities, ServerInfo},
};
use rmcp_macros::{tool, tool_handler, tool_router};
use std::future::Future;
use std::sync::Arc;

use crate::config::Config;
use crate::error::UnifiedIntelligenceError;
use crate::handlers::ToolHandlers;
use crate::handlers::help::{HelpHandlerTrait, UiHelpParams};
use crate::handlers::knowledge::KnowledgeHandler;
use crate::handlers::recall::UiRecallParams;
use crate::handlers::thoughts::ThoughtsHandler;
use crate::models::UiKnowledgeParams;
use crate::models::UiThinkParams;
use crate::qdrant_service::QdrantServiceTrait;
use crate::rate_limit::RateLimiter;
use crate::redis::RedisManager;
use crate::repository::CombinedRedisRepository;
use crate::tools::ui_context::{UIContextParams, ui_context_impl};
use crate::tools::ui_memory::{UiMemoryParams, ui_memory_impl};
use crate::validation::InputValidator;

/// Main service struct for UnifiedIntelligence MCP server
#[derive(Clone)]
pub struct UnifiedIntelligenceService {
    tool_router: ToolRouter<Self>,
    handlers: Arc<ToolHandlers<CombinedRedisRepository>>,
    rate_limiter: Arc<RateLimiter>,
    instance_id: String,
    config: Arc<Config>,
    #[allow(dead_code)]
    qdrant_service: Arc<dyn QdrantServiceTrait>,
}

impl UnifiedIntelligenceService {
    /// Create a new service instance
    pub async fn new(
        redis_manager: Arc<RedisManager>,
        qdrant_service: Arc<dyn QdrantServiceTrait>,
    ) -> Result<Self, UnifiedIntelligenceError> {
        tracing::info!("Service::new() - Starting initialization");
        // Load configuration
        let config = Arc::new(Config::load().map_err(|e| {
            tracing::error!("Service::new() - Failed to load config: {}", e);
            UnifiedIntelligenceError::Config(format!("Failed to load config: {e}"))
        })?);
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
            redis_manager.clone(),  // Pass redis_manager
            qdrant_service.clone(), // Pass qdrant_service
            config.clone(),         // Pass config
        ));
        tracing::info!("Service::new() - ToolHandlers created");

        tracing::info!("Service::new() - Service initialization complete");
        Ok(Self {
            tool_router: Self::tool_router(),
            handlers,
            rate_limiter,
            instance_id,
            config,
            qdrant_service,
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
        description = "Store UI context (session-summaries|important|federation) or return help"
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
