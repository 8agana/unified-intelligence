use std::sync::Arc;
use std::future::Future;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, Content, ErrorData, ServerCapabilities, ServerInfo},
    ServerHandler,
};
use rmcp_macros::{tool, tool_handler, tool_router};
use tracing;

use crate::error::UnifiedIntelligenceError;
use crate::models::{UiThinkParams, UiIdentityParams, UiSearchParams, UiSamParams};
use crate::config::Config;
use crate::redis::RedisManager;
use crate::repository::RedisThoughtRepository;
use crate::handlers::{ToolHandlers, identity::IdentityHandler, sam::SamHandler, search::SearchHandler, thoughts::ThoughtsHandler};
use crate::search_optimization::SearchCache;
use crate::validation::InputValidator;
use crate::rate_limit::RateLimiter;

/// Main service struct for UnifiedIntelligence MCP server
#[derive(Clone)]
pub struct UnifiedIntelligenceService {
    tool_router: ToolRouter<Self>,
    handlers: Arc<ToolHandlers<RedisThoughtRepository>>,
    rate_limiter: Arc<RateLimiter>,
    instance_id: String,
    config: Arc<Config>,
}

impl UnifiedIntelligenceService {
    /// Create a new service instance
    pub async fn new() -> Result<Self, UnifiedIntelligenceError> {
        // Load configuration
        let config = Arc::new(Config::load().map_err(|e| 
            UnifiedIntelligenceError::Configuration(format!("Failed to load config: {}", e))
        )?);
        
        // Get instance ID from environment or config
        let instance_id = std::env::var("INSTANCE_ID")
            .unwrap_or_else(|_| config.server.default_instance_id.clone());
        tracing::info!("Initializing UnifiedIntelligence service for instance: {}", instance_id);
        
        // Initialize Redis with config
        let redis_manager = Arc::new(RedisManager::new_with_config(&config).await?);
        
        // Store OPENAI_API_KEY in Redis if available
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            if !api_key.is_empty() {
                redis_manager.store_api_key("openai_api_key", &api_key).await?;
                tracing::info!("Stored OPENAI_API_KEY in Redis");
            }
        } else {
            tracing::warn!("OPENAI_API_KEY not found in environment");
        }
        
        // Initialize Bloom filter for this instance - DISABLED (requires RedisBloom)
        // redis_manager.init_bloom_filter(&instance_id).await?;
        
        // Initialize event stream for this instance
        redis_manager.init_event_stream(&instance_id).await?;
        
        // Check for search capability
        let search_available = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let search_enabled = redis_manager.create_search_index().await?;
        search_available.store(search_enabled, std::sync::atomic::Ordering::SeqCst);
        
        // Create search cache with configured TTL
        let search_cache = Arc::new(std::sync::Mutex::new(SearchCache::new(config.search.cache_ttl_seconds)));
        
        // Create repository with cache and config
        let repository = Arc::new(RedisThoughtRepository::new(
            redis_manager.clone(),
            search_available.clone(),
            search_cache.clone(),
            instance_id.clone(),
            config.clone(),
        ));
        
        // Create validator
        let validator = Arc::new(InputValidator::new());
        
        // Create rate limiter with configured values
        let rate_limiter = Arc::new(RateLimiter::new(
            config.rate_limiter.max_requests as usize,
            config.rate_limiter.window_seconds as u64
        ));
        
        // Create handlers
        let handlers = Arc::new(ToolHandlers::new(
            repository,
            instance_id.clone(),
            validator,
            search_cache,
            search_available,
        ));
        
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
                format!("Rate limit exceeded. Please slow down your requests."), 
                None
            ));
        }
        
        match self.handlers.ui_think(params.0).await {
            Ok(response) => {
                let content = Content::json(response)
                    .map_err(|e| ErrorData::internal_error(format!("Failed to create JSON content: {}", e), None))?;
                Ok(CallToolResult::success(vec![content]))
            },
            Err(e) => {
                match &e {
                    UnifiedIntelligenceError::DuplicateThought { .. } => {
                        tracing::warn!("Duplicate thought attempted: {}", e);
                        Err(ErrorData::invalid_params(e.to_string(), None))
                    },
                    _ => {
                        tracing::error!("ui_think error: {}", e);
                        Err(ErrorData::internal_error(e.to_string(), None))
                    }
                }
            }
        }
    }
    
    #[tool(description = "Search for thoughts and chains with enhanced filtering capabilities")]
    pub async fn ui_search(
        &self,
        params: Parameters<UiSearchParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                format!("Rate limit exceeded. Please slow down your requests."), 
                None
            ));
        }
        
        match self.handlers.ui_search(params.0).await {
            Ok(response) => {
                let content = Content::json(response)
                    .map_err(|e| ErrorData::internal_error(format!("Failed to create JSON content: {}", e), None))?;
                Ok(CallToolResult::success(vec![content]))
            },
            Err(e) => {
                tracing::error!("ui_search error: {}", e);
                Err(ErrorData::internal_error(e.to_string(), None))
            }
        }
    }
    
    #[tool(description = "View and manage persistent identity through structured categories")]
    pub async fn ui_identity(
        &self,
        params: Parameters<UiIdentityParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                format!("Rate limit exceeded. Please slow down your requests."), 
                None
            ));
        }
        
        match self.handlers.ui_identity(params.0).await {
            Ok(response) => {
                let content = Content::json(response)
                    .map_err(|e| ErrorData::internal_error(format!("Failed to create JSON content: {}", e), None))?;
                Ok(CallToolResult::success(vec![content]))
            },
            Err(e) => {
                tracing::error!("ui_identity error: {}", e);
                Err(ErrorData::internal_error(e.to_string(), None))
            }
        }
    }
    
    #[tool(description = "Manage Sam's context and identity data in Redis")]
    pub async fn ui_sam(
        &self,
        params: Parameters<UiSamParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        // Check rate limit
        if let Err(e) = self.rate_limiter.check_rate_limit(&self.instance_id).await {
            tracing::warn!("Rate limit hit for instance {}: {}", self.instance_id, e);
            return Err(ErrorData::invalid_params(
                format!("Rate limit exceeded. Please slow down your requests."), 
                None
            ));
        }
        
        match self.handlers.ui_sam(params.0).await {
            Ok(response) => {
                let content = Content::json(response)
                    .map_err(|e| ErrorData::internal_error(format!("Failed to create JSON content: {}", e), None))?;
                Ok(CallToolResult::success(vec![content]))
            },
            Err(e) => {
                tracing::error!("ui_sam error: {}", e);
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
            instructions: Some("UnifiedIntelligence MCP Server for Redis-backed thought storage".into()),
        }
    }
}