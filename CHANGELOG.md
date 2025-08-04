# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

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

