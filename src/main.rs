use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

use crate::error::UnifiedIntelligenceError;

mod circuit_breaker;
mod config;
mod embeddings; // New module
mod error;
mod frameworks;
mod handlers;
mod intent;
mod lua_scripts;
mod models;
mod qdrant_service; // New module
mod rate_limit;
mod redis;
mod repository;
mod repository_traits;
mod retry;
mod service;
mod synth;
mod tools;
mod transport;
mod validation;
mod visual;

use crate::config::Config; // Import Config
use crate::qdrant_service::QdrantService;
use crate::redis::RedisManager;
use crate::service::UnifiedIntelligenceService;
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
        UnifiedIntelligenceError::Config(format!("Failed to load config: {e}"))
    })?);

    // Initialize RedisManager
    let redis_manager = Arc::new(RedisManager::new_with_config(&config).await?);

    // Initialize QdrantService
    let instance_id =
        std::env::var("INSTANCE_ID").unwrap_or_else(|_| config.server.default_instance_id.clone());
    let qdrant_service = QdrantService::new(&instance_id).await?;

    let service =
        UnifiedIntelligenceService::new(redis_manager.clone(), Arc::new(qdrant_service)).await?;

    tracing::info!("main: Service created, starting server on stdio transport");
    let server = service.serve(stdio()).await?;
    tracing::info!("main: Server started, waiting for connection to close");

    // This keeps the server running until the transport closes
    server.waiting().await?;
    tracing::info!("main: Server connection closed");

    eprintln!("Server shutting down");
    Ok(())
}
