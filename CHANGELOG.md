# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Remote MCP over Streamable HTTP - 2025-08-12
- Added streamable HTTP server option using `rmcp` 0.5.0 + Axum.
- New env vars: `UI_TRANSPORT` (`stdio` default or `http`), `UI_HTTP_BIND`, `UI_HTTP_PATH`, `UI_BEARER_TOKEN`.
- For headerless clients (Claude Desktop), support `?access_token=<token>` query param.
- Cloudflare tunnel docs and config updated to route public hostname to local bind.
- Maintains backwards-compatible stdio transport for local development.

### Defaults
- Changed default instance id to `DT` (was `CC`).

### ui_remember completion, hybrid scoring, and config presets - 2025-08-10
- Implemented Redis-only `ui_remember` conversational memory with T1→T2→T3 flow (user, assistant synthesis, metrics).
- Added objective feedback loop stored at `voice:feedback:{thought2_id}` with fields: synthesis_quality, continued, abandoned, corrected, time_to_next, feedback_score.
- Hybrid retrieval: combined text search over thoughts and KNN via RediSearch across indices `idx:{instance}:session-summaries`, `idx:{instance}:important`, and `idx:Federation:embeddings`.
- KNN score parsing: use `RETURN 1 score` and convert vector distance to similarity via 1/(1+distance); incorporated into hybrid ranking with recency.
- Config-driven weights: added `ui_remember.hybrid_weights` and presets in `config.yaml`; env overrides (`UI_REMEMBER_*`) supported. Preset is applied last and overrides weights.
- New docs: `docs/ui_remember_config.md` detailing presets, overrides, precedence, and local check commands.
- Safety: removed a production unwrap in FT.SEARCH parsing; added tests for score parsing and feedback heuristic.
- CI workflow: added `.github/workflows/security.yml` (fmt, clippy -D warnings, tests, plus non-blocking cargo audit/deny).
- Added `.env.example` with common env variables and ui_remember overrides.

### UI Context TTL Behavior - 2025-08-10
- Default behavior change: ui_context embedding hashes now have no expiration by default.
- New option: callers can set ttl_seconds > 0 to set an expiry; omitting ttl_seconds leaves keys persistent.
- Notes: existing keys unaffected; no changes to other tools.
- Verification: cargo build, cargo clippy -- -D warnings, cargo fmt -- --check all pass.

### Build + MCP Tool Wiring - 2025-08-10
- Added `ui_context` MCP tool endpoint in `src/service.rs` using `#[tool]`, wiring it to `tools::ui_context::ui_context_impl` with rate limiting and JSON result handling.
- Exposed missing modules in `src/lib.rs` to fix unresolved imports (`pub mod redis;`, `pub mod lua_scripts;`).
- Fixed compile errors in `src/tools/ui_context.rs`:
  - Explicit byte cast for embeddings: `let vector_bytes: Vec<u8> = bytemuck::cast_slice(&vector_f32).to_vec();`.
  - Specified unit return type for `FT.CREATE` query to satisfy `FromRedisValue`.
- Declared `mod tools;` in `src/main.rs` to make `crate::tools` visible to the bin crate.
- Cleaned unused imports in `src/tools/ui_context.rs` and addressed clippy warning (`uninlined_format_args`) in `src/service.rs`.
- Ensured repo passes: `cargo build`, `cargo clippy -- -D warnings`, and `cargo fmt -- --check`.

Notes
- Unit tests currently emit warnings and one failure due to direct construction of `RedisManager` using private fields in `src/handlers/test_handlers.rs`. Consider updating tests to use a constructor, a trait-based mock, or helper factory instead of struct literal initialization.

### CRITICAL MCP PROTOCOL FIX - 2025-08-04
- **FIXED "Unexpected token 'F'" ERROR**: Resolved DT connection failure that has been blocking usage for days
  - Qdrant client was outputting "Failed to obtain server version" to stdout before MCP handshake
  - Added `skip_compatibility_check()` to suppress Qdrant version check output
  - MCP protocol requires clean stdout for JSON-RPC communication
  - This fix allows DT to properly connect to UnifiedIntelligence again

### CRITICAL STABILITY FIXES (Socks CCR Phase 1) - 2025-08-04
- **ELIMINATED RUNTIME PANICS**: Removed all 8 `.unwrap()/.expect()` calls that could crash system at 3AM
  - Fixed in config.rs, qdrant_service.rs, transport.rs, and test files
  - Added proper API key validation to prevent startup failures
- **FIXED DoS VULNERABILITY**: GroqTransport infinite retry loop capped at 5 attempts/5 minutes with jitter
  - Added MAX_RETRIES (5) and MAX_RETRY_DURATION (5 minutes) constants
  - Implemented exponential backoff with jitter (0.8-1.2x multiplier)
  - Added circuit breaker logic to prevent indefinite blocking
- **ENHANCED ERROR HANDLING**: All panics replaced with proper `UnifiedIntelligenceError` propagation
  - Replaced panics with Result<T, E> pattern throughout
  - Added comprehensive error types for all failure modes
- **PRODUCTION SAFETY**: System can now handle failures gracefully without crashing
  - Verified Redis operations are already async (no blocking calls)
  - System remains responsive under all error conditions
- **CODE QUALITY**: Fixed major clippy warnings and formatting issues
  - Fixed uninlined format args throughout codebase
  - Resolved all compilation errors and warnings

### Added
- Centralized .env configuration support via dotenvy
  - Loads from /Users/samuelatagana/Projects/LegacyMind/.env for all API keys
  - Multiple path checking for flexible working directory support
  - Eliminates need for command-line environment variables
- Unified metadata architecture for thought storage
- Display trait implementation for ThinkingFramework enum
- Flexible integer deserialization for MCP client compatibility (DT, CC, Warp)
- Complete metadata embedding pipeline to Qdrant
- Support for framework, importance, relevance, category, and tags in thought records

### Changed
- Updated Rust edition to 2024
- Unified ThoughtRecord structure to include all metadata fields
- Replaced split ThoughtRecord/ThoughtMetadata architecture with single unified record
- Modified embedding pipeline to use unified ThoughtRecord from unified_intelligence crate
- Updated all ThoughtRecord::new calls to include metadata parameters

### Deprecated
- Split ThoughtRecord/ThoughtMetadata architecture (replaced with unified structure)

### Removed
- Separate ThoughtMetadata storage in Redis (now unified in ThoughtRecord)
- External dependency on separate metadata structures in embedding pipeline

### Fixed
- **CRITICAL STABILITY FIXES** (Socks CCR Phase 1):
  - **DoS Vulnerability**: Fixed infinite retry loop in GroqTransport that could block for 3+ hours
    - Added MAX_RETRIES (5 attempts) and MAX_RETRY_DURATION (5 minutes) constants
    - Implemented exponential backoff with jitter to prevent thundering herd
    - Added circuit breaker logic to prevent indefinite blocking
  - **Runtime Panics**: Eliminated 8 `.unwrap()/.expect()` calls that could crash at runtime
    - Replaced with proper error propagation using `UnifiedIntelligenceError`
    - Added graceful error handling for datetime operations in qdrant_service.rs
    - Enhanced config validation to prevent API key issues at startup
  - **Production Safety**: All blocking operations now have proper timeout and fallback handling
- Critical metadata embedding failure - metadata now properly flows to Qdrant
- Self-dependency compilation errors by adding missing module declarations
- ThinkingFramework Display trait implementation for framework string conversion
- MCP client compatibility issues with integer parameter serialization
- Auto-generated thought creation calls to include all required metadata fields
- Ensured `icon` variable is used in `display_framework_start` function
- Removed "sequential" from framework validation error message (framework not actually implemented)

### Security
- 

## [2.0.0] - 2025-08-01
### Added
- Initial release of unified-intelligence (Model Context Protocol MCP) using rmcp 0.3.0.

```
- Replace, add, or remove sections as appropriate for your project.
- For each new release, copy the "Unreleased" section, change the version/date, and start a new "Unreleased" section.
