# ui_remember Tool Documentation

## Overview

The `ui_remember` tool is a conversational memory system that replaces the deprecated `remember`/`deepremember` frameworks from `ui_think`. It provides Redis-backed persistent conversation storage with hybrid retrieval, Groq synthesis, and behavior feedback tracking.

## Features

### Core Functionality
- **Persistent Conversation Storage**: All thoughts stored in Redis with chain tracking
- **Hybrid Retrieval**: Combines embedding-based KNN search and text search
- **Groq Synthesis**: Uses Groq LLMs for response generation with automatic model selection
- **Behavior Feedback**: Tracks user interactions to measure synthesis quality
- **Token Usage Tracking**: Reports token consumption from Groq API
- **Abandon Detection**: Marks conversations as abandoned after 10+ minutes of inactivity

### Implementation Details

#### Thought Storage Structure
Each ui_remember call creates 3 thoughts:
1. **User Query** (category: `ui_remember:user`)
2. **Assistant Response** (category: `ui_remember:assistant`) 
3. **System Metrics** (category: `ui_remember:metrics`)

#### Retrieval System
The hybrid retrieval system combines:
- **Text Search**: Redis FT.SEARCH over thought indices
- **Embedding KNN**: OpenAI embeddings searched across:
  - `idx:{instance}:session-summaries`
  - `idx:{instance}:important`
  - `idx:Federation:embeddings`

#### Ranking Algorithm
Candidates are scored using:
```
combined_score = 0.6 * semantic_score + 0.25 * text_score + 0.15 * recency_score
```
Where recency uses exponential decay with τ = 86400 seconds (1 day).

#### Model Selection Heuristic
- **Fast Model** (`llama3-8b-8192`): Default, used for context size ≤ 8
- **Deep Model** (`llama3-70b-8192`): Used for larger contexts or when style="deep"

#### Feedback Tracking
On each follow-up turn, the system updates the previous assistant response with:
- **continued**: 1 if user continued conversation
- **time_to_next**: Seconds until next user turn
- **corrected**: User's correction text if detected
- **synthesis_quality**: Score based on timing and content
- **feedback_score**: Overall quality score (0.0-1.0)
- **abandoned**: 1 if no follow-up for 600+ seconds

Feedback scoring algorithm:
- Base score from response time: <30s = 0.9, <120s = 0.7, else 0.5
- +0.1 for positive acknowledgments (thanks, got it, etc.)
- -0.1 for corrections (actually, wrong, etc.)
- Minimum 0.3 if correction detected

## API

### Request Parameters

```rust
pub struct UiRememberParams {
    pub thought: String,              // User's query/thought
    pub thought_number: i32,          // Position in conversation
    pub total_thoughts: i32,          // Expected total thoughts
    pub chain_id: Option<String>,     // Conversation chain ID
    pub next_thought_needed: bool,    // Whether more thoughts expected
    
    // Optional parameters (may be ignored)
    pub search_type: Option<String>,  // Search strategy hint
    pub top_k: Option<u32>,          // Max candidates (default: 5)
    pub similarity_threshold: Option<f32>,
    pub token_cap: Option<i32>,
    pub style: Option<String>,       // "deep" for complex queries
    pub tags: Option<Vec<String>>,   // Metadata tags
    pub temporal: Option<String>,    // Time filter hint
}
```

### Response Format

```rust
pub struct UiRememberResult {
    pub status: String,                    // "ok" on success
    pub thought1_id: String,              // User thought ID
    pub thought2_id: String,              // Assistant response ID
    pub thought3_id: Option<String>,      // Metrics thought ID
    pub model_used: Option<String>,       // Groq model used
    pub usage_total_tokens: Option<i32>,  // Total tokens consumed
}
```

## Usage Examples

### Basic Query
```json
{
  "thought": "What is the capital of France?",
  "thought_number": 1,
  "total_thoughts": 1,
  "chain_id": "conv-123"
}
```

### Follow-up with Positive Feedback
```json
{
  "thought": "Thanks! Tell me more about Paris.",
  "thought_number": 2,
  "total_thoughts": 3,
  "chain_id": "conv-123",
  "next_thought_needed": true
}
```

### Complex Query with Deep Model
```json
{
  "thought": "Explain the architectural history of Paris",
  "thought_number": 1,
  "total_thoughts": 1,
  "chain_id": "arch-discussion",
  "style": "deep",
  "top_k": 10,
  "tags": ["architecture", "history"]
}
```

## Redis Data Structures

### Thought Record
```
thought:{instance}:{chain_id}:{thought_id}
  - id: UUID
  - instance: Instance ID
  - thought: Content
  - timestamp: RFC3339
  - chain_id: Conversation chain
  - category: ui_remember:user/assistant/metrics
  - ...
```

### Feedback Hash
```
voice:feedback:{thought_id}
  - synthesis_quality: float (0.0-1.0)
  - continued: 0/1
  - abandoned: 0/1
  - corrected: correction text or empty
  - time_to_next: seconds to next turn
  - feedback_score: float (0.0-1.0)
```

## Configuration

Required environment variables:
- `OPENAI_API_KEY`: For embedding generation
- `GROQ_API_KEY`: For synthesis (or in config)

Config file settings:
```toml
[groq]
api_key = "..."
model_fast = "llama3-8b-8192"
model_deep = "llama3-70b-8192"

[openai]
embedding_model = "text-embedding-3-small"
embedding_dimensions = 1536
```

## Testing

The implementation includes comprehensive tests covering:
- Basic conversation flow
- Feedback tracking and scoring
- Abandon detection
- Correction handling
- Serialization/deserialization

Run tests with:
```bash
cargo test --lib
```

## Performance Considerations

- **Embedding Generation**: ~100-200ms per query
- **KNN Search**: ~50-100ms per index
- **Groq Synthesis**: 500-2000ms depending on model
- **Total Latency**: Typically 1-3 seconds end-to-end

## Future Enhancements

Potential improvements:
- Background task for proactive abandon marking
- Streaming synthesis for lower perceived latency
- Multi-turn context window management
- Adaptive model selection based on query complexity
- Integration with external knowledge bases
- Fine-tuned scoring based on user preferences
