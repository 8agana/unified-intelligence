use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber;

mod models;
mod error;
mod config;
mod redis;
mod repository;
mod repository_traits;
mod handlers;
mod service;
mod search_optimization;
mod validation;
mod rate_limit;
mod lua_scripts;
mod visual;
mod frameworks;
mod identity_documents;
mod circuit_breaker;
mod retry;
mod redis_abstraction;
mod search_enhancements;

use crate::service::UnifiedIntelligenceService;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr for MCP compatibility
    tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();
    
    let service = UnifiedIntelligenceService::new().await?;
    
    // Start the MCP server on stdio transport
    let server = service.serve(stdio()).await?;
    
    // This keeps the server running until the transport closes
    server.waiting().await?;
    
    eprintln!("Server shutting down");
    Ok(())
}