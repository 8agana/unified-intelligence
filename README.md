# Unified Intelligence

## Badges
<!-- Add relevant badges here (e.g., build status, Rust version, license) -->
![Rust](https://github.com/rust-lang/rust/workflows/Rust/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)

## Table of Contents
- [Introduction](#introduction)
- [Features](#features)
- [Tools](#tools)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Configuration](#configuration)
- [Usage](#usage)
  - [Remote MCP (HTTP)](#remote-mcp-http)
  - [Code Examples](#code-examples)
- [Protocol Overview](#protocol-overview)
- [Architecture](#architecture)
- [Extending and Customizing](#extending-and-customizing)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)
- [Links](#links)

## Introduction
Unified Intelligence is a Rust-based Model Context Protocol (MCP) server. It stores and recalls structured “thoughts”, supports multiple thinking frameworks, and exposes additional memory/context tools over stdio or HTTP. Storage is Redis-only, with OpenAI embeddings and Groq for intent parsing and synthesis.

## Definitions
- **Framework (WorkflowState):** A top-level operating mode chosen by the user or system. Examples: `conversation` (default), `debug`, `build`, `stuck`, `review`. Exactly one framework is active at a time; it defines how the system structures interaction.
- **ThinkingMode:** A sub-layer analysis technique available inside a framework. Examples: `first_principles`, `ooda`, `systems`, `root_cause`, `swot`. Multiple modes can be active simultaneously; they are internal “voices/lenses” that can be surfaced in output when helpful.
- **ThinkingSet:** A collection of ThinkingModes active within a given framework. Implemented as `EnumSet<ThinkingMode>`, serialized as a transparent JSON array with deterministic ordering for predictable iteration.

## Features
- **Thinking Frameworks:** Implements multiple cognitive frameworks (OODA, Socratic, First Principles, Systems, Root Cause, SWOT) to guide thought processing for `ui_think`.
- **Groq Integration:** Leverages Groq's powerful language models for natural language understanding, search query parsing, and intelligent response synthesis from retrieved memories.
- **Conversational Memory (Redis-only):** `ui_remember` stores and retrieves all memory in Redis, performing hybrid retrieval (text + KNN via RediSearch) and recording objective feedback metrics.
  
  Redis-only storage: This project does not use Qdrant.
- **Embeddings (OpenAI):** Generates vector embeddings for thoughts and queries using OpenAI's embedding models, enabling semantic search.
- **Redis Persistence:** Stores thought records, chain metadata, and feedback loop metadata in Redis for efficient data management.
- **Extensible Design:** Modular architecture allows for easy extension with new frameworks, integrations, and functionalities.
- **Visual Feedback:** Provides real-time visual feedback on thought processing and framework application.

## Tools
Unified Intelligence is built with and integrates the following:
- **Rust:** The core programming language for performance and safety.
- **`rmcp`:** Rust Model Context Protocol SDK for MCP communication (stdio or streamable HTTP).
- **`tokio`:** Asynchronous runtime for efficient I/O operations.
- **Groq API:** For advanced language model capabilities.
- Redis-only: no Qdrant dependency.
- **Redis:** In-memory data store for thought persistence and metadata.
- **OpenAI API:** For generating vector embeddings.
- **`tracing`:** For structured logging and observability.
- **`colored`:** For enhanced terminal output.

### Version Compatibility
- **`rmcp`:** 0.5.x
- **MCP Protocol:** 2024-11-05

## MCP Tools
- `ui_think`: Capture/process a thought, optionally chained, with an optional thinking framework.
- `ui_recall`: Retrieve a single thought by ID or all thoughts in a chain.
- `ui_help`: Built-in usage and examples for tools and frameworks.
- `ui_knowledge`: Manage entities and relations in a simple knowledge graph (Redis-backed).
- `ui_context`: Store short-lived personal/federation context with embeddings and RediSearch indexing.
- `ui_memory`: Search/read/update/delete memory across embeddings and text with simple filters.
- `ui_remember`: Conversational memory flow: T1 user thought -> T2 assistant synthesis -> T3 objective feedback metrics.
  - Examples below show `next_action` contract for smooth chaining.

## Getting Started

### Prerequisites
- Rust (1.88.0 or later)
- Cargo (Rust's package manager)
- Docker (for running Redis)
- Groq API Key (set as `GROQ_API_KEY` environment variable)
- OpenAI API Key (set as `OPENAI_API_KEY` environment variable)

### Installation
1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-repo/unified-intelligence.git
    cd unified-intelligence
    ```
2.  **Set up Redis (using Docker Compose):**
    ```bash
    # Assuming you have a docker-compose.yml in your Memory directory
    # Navigate to the Memory directory and start services
    cd ../Memory/docker # Adjust path if necessary
    docker-compose up -d
    ```
3.  **Build the project:**
    ```bash
    cargo build --release
    ```
4.  **Run tests (optional, gated to avoid live deps):**
    ```bash
    cargo test
    ```

### Configuration
Unified Intelligence relies on environment variables for configuration:
- `GROQ_API_KEY`: Your API key for Groq services.
- `OPENAI_API_KEY`: Your API key for OpenAI embedding services.
- (Qdrant variables removed; not used.)
- `REDIS_HOST`: Host for your Redis instance (default: `localhost`).
- `REDIS_PORT`: Port for your Redis instance (default: `6379`).
- `INSTANCE_ID`: Instance namespace for storage (default: `DT`).

Remote MCP (HTTP) controls:
- `UI_TRANSPORT=http` to enable HTTP transport (stdio is default otherwise).
- `UI_HTTP_BIND` (e.g., `127.0.0.1:8787`) and `UI_HTTP_PATH` (default `/mcp`).
- `UI_BEARER_TOKEN` to require `Authorization: Bearer <token>`; for headerless clients, `?access_token=<token>` in the URL is supported.
  
For `ui_remember` hybrid retrieval, you can tune weights via config/env (see sections below).

## Security & Checks

- Local checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy -- -D warnings`
  - `cargo test`
  - `cargo audit` (optional; install with `cargo install cargo-audit`)
  - Optionally `cargo deny check` if you have a `deny.toml`

- CI: See `.github/workflows/security.yml`, which runs fmt, clippy, tests, and non-blocking `cargo audit`/`cargo deny`.

## Env & ui_remember Overrides

Copy `.env.example` to `.env` and set:

```
GROQ_API_KEY=…
OPENAI_API_KEY=…
INSTANCE_ID=CC

# ui_remember: preset overrides weights when set
UI_REMEMBER_PRESET=balanced-default
UI_REMEMBER_WEIGHT_SEMANTIC=0.6
UI_REMEMBER_WEIGHT_TEXT=0.25
UI_REMEMBER_WEIGHT_RECENCY=0.15
```

More details in `docs/ui_remember_config.md` including available presets and precedence.

## Usage
Unified Intelligence operates as an MCP server. You can interact with it using an MCP client (e.g., Claude Desktop, or a custom client built with `rmcp`).

### Remote MCP (HTTP)
- Start:
  - `UI_TRANSPORT=http UI_HTTP_BIND=127.0.0.1:8787 UI_HTTP_PATH=/mcp UI_BEARER_TOKEN=<token> ./target/release/unified-intelligence`
- Through Cloudflare Tunnel:
  - Route `mcp.samataganaphotography.com -> http://localhost:8787` in `~/.cloudflared/config.yml`.
- Claude Desktop URL-only setup:
  - `https://mcp.samataganaphotography.com/mcp?access_token=<token>`
- Health check:
  - `https://mcp.samataganaphotography.com/health`

### Quick Start
- Stdio (default): `./target/release/unified-intelligence`
- HTTP: `UI_TRANSPORT=http UI_HTTP_BIND=127.0.0.1:8787 ./target/release/unified-intelligence`

### Code Examples

- ui_remember — action="query"
```
{
  "tool": "ui_remember",
  "params": {
    "action": "query",
    "thought": "Summarize design risks for the frameworks refactor",
    "tags": ["design", "risk"]
  }
}
```

- ui_remember — action="feedback"
```
{
  "tool": "ui_remember",
  "params": {
    "action": "feedback",
    "chain_id": "remember:…",
    "feedback": "Good summary; missing rollout checks and runbook links.",
    "continue_next": true
  }
}
```

- next_action contract returned after query
```
{
  "next_action": {
    "tool": "ui_remember",
    "action": "feedback",
    "required": ["chain_id", "feedback"],
    "optional": ["continue_next"]
  }
}
```

Notes:
- Default `action` is `query`. Omit `chain_id` and the server mints `remember:UUID`.
- After `feedback`, set `continue_next=true` to suggest another `query`.
- When `framework_state="stuck"` in `ui_think`, include `chain_id` to enable per-chain StuckTracker persistence and automatic rotation of thinking modes.

## Protocol Overview
Unified Intelligence implements the Model Context Protocol (MCP), enabling structured communication with AI agents. It exposes tools for:
- **`ui_think`:** The primary tool for processing thoughts through various frameworks.
- **`ui_recall`:** Retrieve a single thought or a chain’s thoughts.
- Additional tools listed above in MCP Tools.

## Architecture
Unified Intelligence follows a modular architecture:
 - `main.rs`: Entry point, initializes the MCP server and handles stdio/HTTP transport.
 - `service.rs`: Tool router via `rmcp_macros`; rate limiting; delegates to handlers and tools.
 - `handlers/`: MCP tool handlers (`thoughts.rs`, `recall.rs`, `help.rs`, `knowledge.rs`).
 - `frameworks.rs`: Thinking frameworks and visuals used by `ui_think`.
 - `transport.rs`: Groq transport client; retries/backoff.
 - `synth.rs` and `intent.rs`: Groq synthesis and intent parsing.
 - `embeddings.rs`: OpenAI embedding generation utilities.
 - `repository*.rs` and `redis.rs`: Redis-backed repositories and managers.
 - `models.rs`, `error.rs`, `validation.rs`, `visual.rs`: Core types, errors, validation, and TTY visuals.

## Extending and Customizing
- **Adding New Thinking Frameworks:** Define new variants in `ThinkingFramework` enum (`frameworks.rs`) and implement their processing logic in `FrameworkProcessor`.
- **Integrating New LLMs/APIs:** Create new client modules similar to `groq.rs` and integrate them into the relevant handlers.
- **Customizing Data Storage:** Implement the `ThoughtRepository` trait (`repository.rs`) with a new backend.
- **Adding New MCP Tools:** Create new handler modules in `handlers/` and register them with the MCP server.

## Testing
Run unit and integration tests using Cargo:
```bash
cargo test
```

## Troubleshooting
- **API Key Errors:** Ensure `GROQ_API_KEY` and `OPENAI_API_KEY` environment variables are correctly set.
- **Connection Issues:** Verify that Redis is running and accessible on the configured host and port. Check firewall settings if necessary.
- **Compilation Errors:** Refer to the Rust compiler's error messages for guidance. Use `cargo fix` and `cargo check` for assistance.
- **Unexpected Behavior:** Review `tracing` logs for detailed insights into application flow and potential issues.

### Known Issues
- Docs may lag new help content periodically; use `ui_help` with `tool` and `topic` for up-to-date parameters and examples.
- Tests are designed to skip live Redis if unavailable, but logs may mention connection attempts.

## Links
- [CHANGELOG.md](CHANGELOG.md)
- [FIXME.md](FIXME.md)
- [BUILDME.md](BUILDME.md)
