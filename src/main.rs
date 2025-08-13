use anyhow::Result;
use rmcp::{
    ServiceExt,
    transport::{
        stdio,
        streamable_http_server::tower::{StreamableHttpServerConfig, StreamableHttpService},
    },
};
use std::net::SocketAddr;
use std::time::Duration;

// Axum HTTP server for remote MCP
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
};

mod circuit_breaker;
mod config;
mod embeddings; // New module
mod error;
mod frameworks;
mod handlers;
mod intent;
mod lua_scripts;
mod models;
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
    let config = Arc::new(Config::load());

    // Initialize RedisManager
    let redis_manager = Arc::new(RedisManager::new_with_config(&config).await?);

    // Create service (no Qdrant dependency)
    let service = UnifiedIntelligenceService::new(redis_manager.clone()).await?;

    // Choose transport: stdio (default) or http
    let transport = std::env::var("UI_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());
    match transport.as_str() {
        "http" | "streamable_http" => {
            // Bind address and route path
            let bind: SocketAddr = std::env::var("UI_HTTP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
                .parse()
                .expect("Invalid UI_HTTP_BIND (expected host:port)");
            let path = std::env::var("UI_HTTP_PATH").unwrap_or_else(|_| "/mcp".to_string());

            // Optional bearer token auth (recommended behind Cloudflare Access)
            let bearer_token = std::env::var("UI_BEARER_TOKEN").ok();

            // Create streamable HTTP service using the existing service (Clone)
            let svc_factory_service = service.clone();
            let session_manager: rmcp::transport::streamable_http_server::session::local::LocalSessionManager = Default::default();
            let http_service: StreamableHttpService<UnifiedIntelligenceService, _> =
                StreamableHttpService::new(
                    move || Ok(svc_factory_service.clone()),
                    Arc::new(session_manager),
                    StreamableHttpServerConfig {
                        stateful_mode: true,
                        sse_keep_alive: Some(Duration::from_secs(15)),
                    },
                );

            // Axum router with optional bearer auth middleware
            let mut router = Router::new().nest_service(path.as_str(), http_service);
            if let Some(expected) = bearer_token.clone() {
                let expected = Arc::new(expected);
                router = router.layer(middleware::from_fn_with_state(
                    expected.clone(),
                    require_bearer,
                ));
            }

            // Add a simple health endpoint
            let router = router.route("/health", axum::routing::get(|| async { "ok" }));

            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!(
                %bind,
                path = %path,
                auth = %bearer_token.as_deref().map(|_| "bearer").unwrap_or("none"),
                "Starting Streamable HTTP MCP server"
            );

            axum::serve(listener, router).await?;
            Ok(())
        }
        _ => {
            tracing::info!("main: Service created, starting server on stdio transport");
            let server = service.serve(stdio()).await?;
            tracing::info!("main: Server started, waiting for connection to close");
            server.waiting().await?;
            tracing::info!("main: Server connection closed");
            eprintln!("Server shutting down");
            Ok(())
        }
    }
}

async fn require_bearer(
    State(expected): State<Arc<String>>,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    if req.uri().path().eq("/health") {
        return next.run(req).await;
    }
    let headers: &HeaderMap = req.headers();
    let authorized = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {}", expected.as_str()))
        || {
            // Fallback: allow token via query parameter for clients that cannot set headers
            // Accepted keys: access_token, token
            if let Some(q) = req.uri().query() {
                for pair in q.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if (k == "access_token" || k == "token") && v == expected.as_str() {
                            return next.run(req).await;
                        }
                    }
                }
            }
            false
        };
    if !authorized {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    next.run(req).await
}
