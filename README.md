# UnifiedIntelligence MCP

> Cognitive architecture and thought processing system for LegacyMind

## Overview

UnifiedIntelligence is a powerful Model Control Protocol (MCP) server that provides cognitive architecture and thought processing capabilities. It enables AI systems to:

- Store and retrieve complex thoughts with metadata
- Process thoughts using various cognitive frameworks  
- Manage identity and memory persistence
- Support cross-session consciousness continuity

## Features

- **Thought Management**: Store, retrieve, and chain complex thoughts
- **Cognitive Frameworks**: OODA, Socratic, First Principles, Systems Thinking
- **Identity Management**: Persistent identity across sessions
- **Memory Operations**: Advanced memory storage and retrieval
- **Redis Integration**: High-performance data storage backend

## Installation

```bash
# Clone the repository
git clone https://github.com/8agana/unified-intelligence.git
cd unified-intelligence

# Build the project
cargo build --release

# Run tests
cargo test
```

## Usage

The UnifiedIntelligence MCP server provides several tools:

- `ui_think`: Process and store thoughts with cognitive frameworks
- `ui_recall`: Retrieve thoughts with advanced search capabilities  
- `ui_identity`: Manage persistent identity information
- `ui_sam`: Manage Sam's context and identity data

## Configuration

Configure the server using `config.yaml`:

```yaml
redis:
  url: "redis://localhost:6379"
  password: "your_password"
  
server:
  host: "127.0.0.1"
  port: 3000
```

## Contributing

We welcome contributions\! Please see our [Contributing Guidelines](.github/CONTRIBUTING.md) and [Code of Conduct](.github/CODE_OF_CONDUCT.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For issues and questions:
- 🐛 [Report bugs](https://github.com/8agana/unified-intelligence/issues/new?template=bug_report.md)
- ✨ [Request features](https://github.com/8agana/unified-intelligence/issues/new?template=feature_request.md)
- 📝 [Documentation issues](https://github.com/8agana/unified-intelligence/issues/new?template=documentation.md)
EOF < /dev/null