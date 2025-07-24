# UnifiedIntelligence MCP: The Thought Engine

**A lean, high-performance Multi-Agent Cognitive Primitive (MCP) focused on structured thought capture and retrieval.**

This document provides a comprehensive overview of the refactored UnifiedIntelligence MCP, its architecture, and instructions for setup, usage, and development.

## Table of Contents

- [Project Overview](#project-overview)
- [Architecture](#architecture)
- [Features](#features)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Configuration](#configuration)
- [Usage](#usage)
  - [Core Tools](#core-tools)
    - [ui_think](#ui_think)
    - [ui_recall](#ui_recall)
  - [Usage Examples](#usage-examples)
- [Development](#development)
  - [Project Structure](#project-structure)
  - [Building from Source](#building-from-source)
  - [Running Tests](#running-tests)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)

## Project Overview

UnifiedIntelligence has been refactored to be a specialized "Thought Engine." Its primary purpose is to provide a robust and efficient foundation for AI agents to:

- **Think in structured ways:** Capture and organize thoughts, ideas, and plans using various cognitive frameworks.
- **Retrieve specific information efficiently:** Store and retrieve individual thoughts and thought chains by their unique identifiers.

This MCP is designed to be a high-speed, write-optimized component that generates the raw stream of consciousness for the entire system. Complex search, identity management, and interaction with external LLMs are handled by other specialized MCPs (e.g., UnifiedRAG).

The system is built in Rust for performance, safety, and concurrency, and it leverages Redis for a fast and flexible data backend.

## Architecture

The UnifiedIntelligence MCP follows a modular, service-oriented architecture.

```
+-------------------------------------------------+
|            UnifiedIntelligence MCP              |
|              (Rust Application)                 |
+-------------------------------------------------+
|         |                 |                 |
|   Tool Interface      Core Logic       Data Access Layer |
|    (ui_think)       (Handlers, Models)   (Repository)    |
+-------------------------------------------------+
|                 |                 |
|        +--------+---------+       |
|        | Redis Abstraction |       |
|        +-----------------+       |
|                 |                 |
+-------------------------------------------------+
|                      Redis                      |
|             (JSON, Streams, etc.)               |
+-------------------------------------------------+
```

### Core Components

- **Tool Interface:** Exposes the core functionalities (`ui_think`, `ui_recall`) to the AI agent.
- **Core Logic:**
  - **Handlers:** Implement the business logic for each tool.
  - **Models:** Define the data structures used throughout the application (e.g., `ThoughtRecord`).
- **Data Access Layer (Repository):** Provides a unified interface for interacting with the Redis database, abstracting away the specific Redis commands.
- **Redis Abstraction:** A lower-level module that communicates directly with Redis, handling connections, commands, and data serialization.
- **Redis:** The primary data store, utilizing various Redis modules for flexibility and performance:
  - **RedisJSON:** For storing structured data like thought metadata.
  - **Redis Streams:** For event logging and inter-component communication.

## Features

- **Structured Thinking:** Use cognitive frameworks (e.g., `OODA`, `Socratic`) to guide the thinking process.
- **Chained Thoughts:** Link thoughts together to form coherent lines of reasoning.
- **Rich Metadata:** Attach importance, relevance, tags, and categories to thoughts for nuanced retrieval.
- **High Performance:** Built in Rust to handle multiple requests concurrently with high performance.
- **Clean Architecture:** Designed to be lean and focused, offloading complex tasks to other specialized MCPs.

## Getting Started

### Prerequisites

- **Rust:** (version 1.60 or later)
- **Redis:** (version 7.0 or later) with RedisJSON module installed.
- **Docker:** (optional, for running Redis)

### Installation

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-repo/unified-intelligence.git
    cd unified-intelligence
    ```

2.  **Build the project:**
    ```bash
    cargo build --release
    ```
    The executable will be located at `target/release/unified-intelligence`.

### Configuration

The application is configured through a `config.yaml` file in the project root and environment variables.

#### `config.yaml`

```yaml
# Example config.yaml (simplified)
server:
  name: "unified-intelligence"
  version: "3.0.0"
  default_instance_id: "test"
redis:
  host: "localhost"
  port: 6379
  database: 0
  pool:
    max_size: 16
    timeout_seconds: 5
    create_timeout_seconds: 5
    recycle_timeout_seconds: 5
  default_ttl_seconds: 604800
rate_limiter:
  max_requests: 100
  window_seconds: 60
event_stream:
  max_length: 10000
  approximate_trimming: true
bloom_filter:
  error_rate: 0.01
  expected_items: 100000
time_series:
  retention_ms: 86400000
  duplicate_policy: "SUM"
retry:
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 5000
  backoff_base: 2.0
  jitter_factor: 0.1
```

#### Environment Variables

- `REDIS_URL`: Overrides the `redis_url` in `config.yaml`.
- `RUST_LOG`: Overrides the `log_level` in `config.yaml`.
- `REDIS_PASSWORD`: Redis password.
- `INSTANCE_ID`: The unique ID for this MCP instance.

## Usage

The MCP is used by calling its various tool endpoints with the appropriate parameters.

### Core Tools

#### `ui_think`

Stores a thought, optionally as part of a chain, and applies a cognitive framework.

- **`thought`**: The content of the thought.
- **`thought_number`**: The current thought's position in a sequence.
- **`total_thoughts`**: The total number of thoughts in the sequence.
- **`next_thought_needed`**: `true` if more thoughts are expected in the sequence.
- **`chain_id`**: (Optional) The ID of the chain this thought belongs to.
- **`framework`**: (Optional) The cognitive framework to apply (e.g., `ooda`, `socratic`).
- **`importance`**, **`relevance`**, **`tags`**, **`category`**: (Optional) Metadata for the thought.

#### `ui_recall`

*This tool is not yet implemented. Its intended functionality is to retrieve thoughts and memories by ID or chain ID.*

### Usage Examples

**Storing a single thought:**
```json
{
  "tool_name": "ui_think",
  "parameters": {
    "thought": "This is a standalone thought.",
    "thought_number": 1,
    "total_thoughts": 1,
    "next_thought_needed": false
  }
}
```

**Starting a chain of thoughts:**
```json
{
  "tool_name": "ui_think",
  "parameters": {
    "thought": "This is the first thought in a new chain.",
    "thought_number": 1,
    "total_thoughts": 3,
    "next_thought_needed": true,
    "chain_id": "my-new-chain-123"
  }
}
```

## Development

### Project Structure

```
.
├── src/
│   ├── handlers/     # Logic for each tool
│   ├── frameworks.rs # Cognitive frameworks
│   ├── models.rs     # Data structures
│   ├── repository.rs # Redis data access
│   ├── redis.rs      # Redis connection and commands
│   └── main.rs       # Main application entry point
├── Cargo.toml
└── README.md
```

### Building from Source

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

## Troubleshooting

- **Connection errors:** Ensure your Redis server is running and accessible at the configured URL.
- **Validation errors:** The MCP performs strict input validation. Check the error messages for details on which field is invalid and why.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.