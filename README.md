# UnifiedIntelligence MCP

**A sophisticated Multi-Agent Cognitive Primitive (MCP) for advanced AI reasoning, memory, and identity management.**

This document provides a comprehensive overview of the UnifiedIntelligence MCP, its architecture, and instructions for setup, usage, and development.

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
    - [ui_identity](#ui_identity)
    - [ui_search](#ui_search)
    - [ui_sam](#ui_sam)
  - [Usage Examples](#usage-examples)
- [Development](#development)
  - [Project Structure](#project-structure)
  - [Building from Source](#building-from-source)
  - [Running Tests](#running-tests)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)

## Project Overview

UnifiedIntelligence is a powerful MCP designed to provide a robust foundation for AI agents. It enables agents to:

- **Think in structured ways:** Capture and organize thoughts, ideas, and plans using various cognitive frameworks.
- **Recall information efficiently:** Store and retrieve memories, conversations, and knowledge with a sophisticated, multi-layered search system.
- **Maintain a persistent identity:** Develop and evolve a unique personality, preferences, and behavioral patterns over time.
- **Manage context effectively:** Keep track of user information, project state, and environmental variables.
- **Interact with a human counterpart:** Provides a dedicated interface (`ui_sam`) for managing the human user's context and preferences.

The system is built in Rust for performance, safety, and concurrency, and it leverages Redis for a fast and flexible data backend.

## Architecture

The UnifiedIntelligence MCP follows a modular, service-oriented architecture.

```
+-------------------------------------------------+
|            UnifiedIntelligence MCP              |
| (Rust Application)                              |
+-------------------------------------------------+
|         |                 |                 |
|   Tool Interface      Core Logic       Data Access Layer |
|  (ui_think, etc.)   (Handlers, Models)   (Repository)    |
+-------------------------------------------------+
|                 |                 |
|        +--------+---------+       |
|        | Redis Abstraction |       |
|        +-----------------+       |
|                 |                 |
+-------------------------------------------------+
|                      Redis                      |
| (JSON, Search, Streams)                         |
+-------------------------------------------------+
```

### Core Components

- **Tool Interface:** Exposes the core functionalities (`ui_think`, `ui_recall`, etc.) to the AI agent.
- **Core Logic:**
  - **Handlers:** Implement the business logic for each tool.
  - **Models:** Define the data structures used throughout the application (e.g., `ThoughtRecord`, `Identity`).
- **Data Access Layer (Repository):** Provides a unified interface for interacting with the Redis database, abstracting away the specific Redis commands.
- **Redis Abstraction:** A lower-level module that communicates directly with Redis, handling connections, commands, and data serialization.
- **Redis:** The primary data store, utilizing various Redis modules for flexibility and performance:
  - **RedisJSON:** For storing structured data like identity and thought metadata.
  - **RediSearch:** For powerful full-text search capabilities.
  - **Redis Streams:** For event logging and inter-component communication.

## Features

- **Structured Thinking:** Use cognitive frameworks (e.g., `Sequential`, `Problem-Solution`) to guide the thinking process.
- **Chained Thoughts:** Link thoughts together to form coherent lines of reasoning.
- **Rich Metadata:** Attach importance, relevance, tags, and categories to thoughts for nuanced recall.
- **Evolving Identity:** The `ui_identity` tool allows the agent to build and modify its own identity, from communication style to technical skills.
- **Human-in-the-Loop:** The `ui_sam` tool provides a dedicated namespace for managing the human user's context, preferences, and identity.
- **Unified Search:** A single `ui_search` endpoint provides access to various search modes, including text-based search and chain retrieval.
- **Visual Feedback:** Provides clear, color-coded visual output for all operations, enhancing the user experience.
- **Error Handling:** Comprehensive error handling with clear and informative messages.
- **Concurrency and Performance:** Built in Rust to handle multiple requests concurrently with high performance.

## Getting Started

### Prerequisites

- **Rust:** (version 1.60 or later)
- **Redis:** (version 7.0 or later) with the following modules installed:
  - **RedisJSON**
  - **RediSearch**
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
redis_url: "redis://127.0.0.1/"
log_level: "info"
```

#### Environment Variables

- `REDIS_URL`: Overrides the `redis_url` in `config.yaml`.
- `RUST_LOG`: Overrides the `log_level` in `config.yaml`.

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
- **`framework`**: (Optional) The cognitive framework to apply (e.g., `sequential`, `problem-solution`).
- **`importance`**, **`relevance`**, **`tags`**, **`category`**: (Optional) Metadata for the thought.

#### `ui_recall`

*This tool is not yet fully implemented in the provided source code, but its intended functionality is to retrieve thoughts and memories.*

#### `ui_identity`

Manages the agent's persistent identity.

- **`operation`**: `view`, `add`, `modify`, `delete`, `help`.
- **`category`**: The category of identity to modify (e.g., `core_info`, `communication`).
- **`field`**: The specific field to modify.
- **`value`**: The new value for the field.

#### `ui_search`

Searches for thoughts and chains.

- **`mode`**: `thought` or `chain`.
- **`query`**: (For `thought` mode) The search query.
- **`chain_id`**: (For `chain` mode) The ID of the chain to retrieve.
- **`limit`**: (Optional) The maximum number of results to return.
- **`search_all_instances`**: (Optional) `true` to search across all agent instances.

#### `ui_sam`

Manages the human user's context and identity. The parameters are similar to `ui_identity`.

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

**Modifying the agent's identity:**
```json
{
  "tool_name": "ui_identity",
  "parameters": {
    "operation": "modify",
    "category": "communication",
    "field": "humor_level",
    "value": 0.8
  }
}
```

**Searching for thoughts:**
```json
{
  "tool_name": "ui_search",
  "parameters": {
    "mode": "thought",
    "query": "persistent identity"
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

- **Connection errors:** Ensure your Redis server is running and accessible at the configured URL. Check that the RedisJSON and RediSearch modules are loaded.
- **Validation errors:** The MCP performs strict input validation. Check the error messages for details on which field is invalid and why.
- **Search issues:** If search results are not as expected, ensure that your RediSearch index is up to date.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.