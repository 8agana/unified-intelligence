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
  - [Code Examples](#code-examples)
- [Protocol Overview](#protocol-overview)
- [Architecture](#architecture)
- [Extending and Customizing](#extending-and-customizing)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)
- [Links](#links)

## Introduction
Unified Intelligence is a Rust-based Model Context Protocol (MCP) server designed to enhance cognitive capabilities by processing and synthesizing thoughts using various thinking frameworks. It integrates with external services like Groq for advanced language model capabilities, OpenAI for embeddings, and Qdrant for vector-based memory storage and retrieval. This project aims to provide a robust and extensible backend for intelligent agents and applications.

## Features
- **Thinking Frameworks:** Implements various cognitive frameworks (OODA, Socratic, First Principles, Systems, Root Cause, SWOT, Remember) to guide thought processing.
- **Groq Integration:** Leverages Groq's powerful language models for natural language understanding, search query parsing, and intelligent response synthesis from retrieved memories.
- **Conversational Memory (Redis-only):** `ui_remember` stores and retrieves all memory in Redis, performing hybrid retrieval (text + KNN via RediSearch) and recording objective feedback metrics.
- **Vector Memory (Qdrant):** Qdrant integration remains available for other features, but `ui_remember` operates Redis-only by design.
- **Embeddings (OpenAI):** Generates vector embeddings for thoughts and queries using OpenAI's embedding models, enabling semantic search.
- **Redis Persistence:** Stores thought records, chain metadata, and feedback loop metadata in Redis for efficient data management.
- **Extensible Design:** Modular architecture allows for easy extension with new frameworks, integrations, and functionalities.
- **Visual Feedback:** Provides real-time visual feedback on thought processing and framework application.

## Tools
Unified Intelligence is built with and integrates the following key technologies:
- **Rust:** The core programming language for performance and safety.
- **`rmcp`:** Rust Model Context Protocol SDK for MCP communication.
- **`tokio`:** Asynchronous runtime for efficient I/O operations.
- **Groq API:** For advanced language model capabilities.
- **Qdrant:** Vector database for semantic memory storage and retrieval.
- **Redis:** In-memory data store for thought persistence and metadata.
- **OpenAI API:** For generating vector embeddings.
- **`tracing`:** For structured logging and observability.
- **`colored`:** For enhanced terminal output.

### Version Compatibility
- **`rmcp` Version:** This project now uses `rmcp` version `0.5.0`.
- **Protocol Version:** Supports protocol version `2024-11-05`.

## Getting Started

### Prerequisites
- Rust (1.88.0 or later)
- Cargo (Rust's package manager)
- Docker (for running Redis and Qdrant)
- Groq API Key (set as `GROQ_API_KEY` environment variable)
- OpenAI API Key (set as `OPENAI_API_KEY` environment variable)

### Installation
1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-repo/unified-intelligence.git
    cd unified-intelligence
    ```
2.  **Set up Redis and Qdrant (using Docker Compose):**
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
4.  **Run tests (optional but recommended):**
    ```bash
    cargo test
    ```

### Configuration
Unified Intelligence relies on environment variables for configuration:
- `GROQ_API_KEY`: Your API key for Groq services.
- `OPENAI_API_KEY`: Your API key for OpenAI embedding services.
- `QDRANT_HOST`: Host for your Qdrant instance (default: `localhost`).
- `QDRANT_PORT`: Port for your Qdrant instance (default: `6334`).
- `REDIS_HOST`: Host for your Redis instance (default: `localhost`).
- `REDIS_PORT`: Port for your Redis instance (default: `6379`).
- `QDRANT_SIMILARITY_THRESHOLD`: (Optional) Threshold for Qdrant search results (default: `0.35`).
  
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

### Code Examples
To start the Unified Intelligence server:
```bash
./target/release/unified-intelligence
```
(Further usage examples would depend on the specific MCP client being used and the tools exposed by the server.)

## Protocol Overview
Unified Intelligence implements the Model Context Protocol (MCP), enabling structured communication with AI agents. It exposes tools for:
- **`ui_think`:** The primary tool for processing thoughts through various frameworks.
- **`ui_recall`:** (Planned/Implemented in `recall.rs`) For retrieving memories based on specific criteria.
- Other custom tools as defined in the `handlers` module.

## Architecture
Unified Intelligence follows a modular architecture:
- **`main.rs`:** Entry point, initializes the MCP server and handlers.
- **`service.rs`:** Orchestrates interactions between different components.
- **`handlers/`:** Contains logic for handling specific MCP tool calls (e.g., `thoughts.rs` for `ui_think`).
- **`frameworks.rs`:** Defines and implements the various thinking frameworks.
- **`groq.rs`:** Encapsulates Groq API interactions.
- **`embeddings.rs`:** Handles OpenAI embedding generation.
- **`qdrant_service.rs`:** Manages connections and operations with the Qdrant vector database.
- **`repository.rs`:** Defines the `ThoughtRepository` trait for data persistence.
- **`redis.rs`:** Implements Redis-specific data storage and retrieval.
- **`models.rs`:** Defines data structures used across the application.
- **`error.rs`:** Custom error types.
- **`validation.rs`:** Input validation logic.
- **`visual.rs`:** Handles terminal output and visual feedback.

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
- **Connection Issues:** Verify that Redis and Qdrant Docker containers are running and accessible on the configured hosts and ports. Check firewall settings if necessary.
- **Compilation Errors:** Refer to the Rust compiler's error messages for guidance. Use `cargo fix` and `cargo check` for assistance.
- **Unexpected Behavior:** Review `tracing` logs for detailed insights into application flow and potential issues.

## Links
- [CHANGELOG.md](CHANGELOG.md)
- [FIXME.md](FIXME.md)
- [BUILDME.md](BUILDME.md)
