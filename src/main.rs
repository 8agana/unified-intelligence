use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber;

use crate::error::UnifiedIntelligenceError;

mod models;
mod error;
mod config;
mod redis;
mod repository;
mod repository_traits;
mod handlers;
mod service;
mod validation;
mod rate_limit;
mod lua_scripts;
mod visual;
mod frameworks;
mod circuit_breaker;
mod retry;
mod embeddings; // New module
mod qdrant_service; // New module
mod intent;
mod synth;
mod transport;

use crate::service::UnifiedIntelligenceService;
use crate::redis::RedisManager;
use crate::qdrant_service::QdrantService;
use crate::config::Config; // Import Config
use std::sync::Arc; // Import Arc

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr for MCP compatibility
    tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();
    
    // Load configuration
    let config = Arc::new(Config::load().map_err(|e| {
        tracing::error!("main: Failed to load config: {}", e);
        UnifiedIntelligenceError::Config(format!("Failed to load config: {}", e))
    })?);

    // Initialize RedisManager
    let redis_manager = Arc::new(RedisManager::new_with_config(&config).await?);

    // Initialize QdrantService
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| config.server.default_instance_id.clone());
    let qdrant_service = QdrantService::new(&instance_id).await?;

    let service = UnifiedIntelligenceService::new(redis_manager.clone(), qdrant_service).await?;
    
    tracing::info!("main: Service created, starting server on stdio transport");
    let server = service.serve(stdio()).await?;
    tracing::info!("main: Server started, waiting for connection to close");
    
    // This keeps the server running until the transport closes
    server.waiting().await?;
    tracing::info!("main: Server connection closed");
    
    eprintln!("Server shutting down");
    Ok(())
}