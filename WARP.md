# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Core Commands

### Build & Run
```bash
# Standard release build (uses rmcp 0.5.0)
cargo build --release

# Run locally with stdio transport (default)
./target/release/unified-intelligence

# Run with HTTP transport for remote MCP
UI_TRANSPORT=http UI_HTTP_BIND=127.0.0.1:8787 UI_HTTP_PATH=/mcp ./target/release/unified-intelligence

# Quick start/stop/restart using provided script
./scripts/ui_mcp.sh start    # Starts with HTTP transport
./scripts/ui_mcp.sh stop     # Stops running instance
./scripts/ui_mcp.sh restart  # Restart service
./scripts/ui_mcp.sh status   # Check if running
```

### Testing & Quality
```bash
# Run all tests
cargo test --all --locked --all-features

# Format check (MUST pass)
cargo fmt --all -- --check

# Clippy linting (warnings are errors)
cargo clippy -- -D warnings

# Security audit (optional, non-blocking)
cargo audit

# Run specific test module
cargo test handlers::test_handlers
cargo test service::tests
```

### Development Workflow
```bash
# Watch for changes and rebuild
cargo watch -x build

# Run with debug logging
RUST_LOG=debug cargo run

# Check a single file before committing
cargo fmt --all
cargo clippy -- -D warnings
```

## Architecture Overview

This is a Rust-based MCP (Model Context Protocol) server implementing the UnifiedIntelligence system for enhanced cognitive processing. It uses **rmcp 0.5.0** patterns with `#[tool(tool_box)]` macros (never `#[tool_router]`).

### Service Architecture Flow
1. **Entry Point** (`main.rs`): Chooses transport (stdio or HTTP) based on `UI_TRANSPORT` env var
2. **Service Layer** (`service.rs`): Main `UnifiedIntelligenceService` with `#[tool_router]` implementing all MCP tools
3. **Handler Layer** (`handlers/`): Business logic for each tool (thoughts, recall, knowledge, help)
4. **Storage Layer**: Redis-only persistence via `CombinedRedisRepository`
5. **Integration Layer**: Groq for synthesis, OpenAI for embeddings

### Critical Patterns

#### rmcp 0.5.0 Tool Implementation
```rust
// Service uses #[tool_router] on impl block
#[tool_router]
impl UnifiedIntelligenceService {
    #[tool(description = "Tool description")]
    pub async fn ui_tool(&self, params: Parameters<Params>) -> Result<CallToolResult, ErrorData> {
        // Rate limiting check
        // Handler delegation
        // JSON response wrapping
    }
}

// Handler trait MUST NOT use tool macros
impl ServerHandler for UnifiedIntelligenceService {
    fn get_info(&self) -> ServerInfo { /* ... */ }
}
```

#### Redis Key Patterns
- Thoughts: `{instance_id}:thoughts:{uuid}`
- Chains: `{instance_id}:chains:{chain_id}`
- Embeddings: `{instance_id}:embeddings:{category}:{uuid}`
- Feedback: `voice:feedback:{thought_id}`
- Entities: `kg:{scope}:entities:{entity_id}`
- RediSearch indices: `idx:{instance_id}:{category}`

### Tool Suite

1. **ui_think**: Process thoughts through frameworks (OODA, Socratic, First Principles, Systems, Root Cause, SWOT)
2. **ui_recall**: Retrieve thoughts by ID or chain ID
3. **ui_help**: Get help for any tool with examples
4. **ui_knowledge**: Manage knowledge graph entities and relationships
5. **ui_context**: Store context with embeddings (personal/federation scopes)
6. **ui_memory**: Search/CRUD operations on embedded memories
7. **ui_remember**: Conversational memory with hybrid retrieval and synthesis

## Dependencies & Environment

### Required Services
1. **Redis** (with RediSearch module)
   ```bash
   cd ../Memory/docker
   docker-compose up -d
   ```

2. **Environment Variables** (copy `.env.example` to `.env`)
   - `GROQ_API_KEY`: For language model synthesis
   - `OPENAI_API_KEY`: For embeddings generation
   - `INSTANCE_ID`: Namespace for storage (default: CC)
   - `UI_TRANSPORT`: stdio (default) or http
   - `UI_HTTP_BIND`: For HTTP transport (e.g., 127.0.0.1:8787)
   - `UI_BEARER_TOKEN`: Optional auth token

### Remote MCP Setup (HTTP Transport)
The service can run behind Cloudflare Tunnel for remote access:
- Local: `http://127.0.0.1:8787/mcp`
- Remote: `https://mcp.samataganaphotography.com/mcp?access_token=<token>`
- Health check: `/health` endpoint

## Key Implementation Details

### Rate Limiting & Circuit Breakers
- Rate limiter per instance_id with configurable windows
- Circuit breaker pattern for external API calls (Groq, OpenAI)
- Retry logic with exponential backoff

### Hybrid Memory Retrieval (ui_remember)
Combines three scoring dimensions with configurable weights:
- **Semantic**: Vector similarity via OpenAI embeddings
- **Text**: Keyword/phrase matching via RediSearch
- **Recency**: Time-based decay scoring

Presets available: `balanced-default`, `fast-chat`, `deep-research`, `recall-recent`

### Transport Modes
- **stdio**: Default for local Claude Desktop integration
- **HTTP**: For remote access, supports SSE streaming and bearer token auth
- Configurable via `UI_TRANSPORT` environment variable

## Common Development Tasks

### Adding a New Tool
1. Define params struct in `models.rs` with `JsonSchema` derive
2. Add tool method to `service.rs` with `#[tool()]` attribute
3. Implement handler logic in `handlers/` module
4. Add rate limiting and validation checks
5. Update help system in `handlers/help.rs`

### Modifying Thinking Frameworks
1. Edit `frameworks.rs` to add/modify framework enum variants
2. Update `FrameworkProcessor::process()` with new logic
3. Add visual feedback in `visual.rs`
4. Update tests in handler modules

### Debugging Remote Issues
```bash
# Check all components
ps aux | grep unified-intelligence
docker ps | grep redis
ps aux | grep cloudflared

# View logs
tail -f /Users/samuelatagana/Library/Logs/unified-intelligence.*.log

# Test endpoints
curl http://127.0.0.1:8787/health
curl https://mcp.samataganaphotography.com/health
```

## CI/CD Pipeline
GitHub Actions workflow (`.github/workflows/security.yml`) runs on all pushes:
1. Format check (`cargo fmt`)
2. Clippy linting (`cargo clippy`)
3. Test suite (`cargo test`)
4. Security audit (`cargo audit` - non-blocking)
5. Dependency check (`cargo deny` - non-blocking)

## Performance Considerations
- Redis operations use connection pooling via `deadpool-redis`
- Embeddings cached to avoid redundant OpenAI API calls
- Lua scripts for atomic Redis operations
- Async/await throughout with `tokio` runtime
- Circuit breakers prevent cascade failures

## Security Notes
- Bearer token auth for HTTP transport (recommended behind Cloudflare Access)
- Query parameter fallback (`?access_token=`) for clients without header support
- Input validation on all tool parameters
- Rate limiting per instance to prevent abuse
- Secrets managed via environment variables, never in code
