use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing;

use crate::error::Result;
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};

/// Parameters for the ui_help tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiHelpParams {
    #[schemars(
        description = "Optional specific tool to get help for ('ui_think', 'ui_recall', or leave empty for general help)"
    )]
    pub tool: Option<String>,

    #[schemars(
        description = "Optional specific topic ('frameworks', 'parameters', 'examples', or leave empty for all)"
    )]
    pub topic: Option<String>,
}

/// Response structure for help requests
#[derive(Debug, Serialize)]
pub struct HelpResponse {
    pub overview: String,
    pub tools: serde_json::Value,
    pub examples: serde_json::Value,
    pub tips: Vec<String>,
}

/// Handler for help operations
pub struct HelpHandler {
    instance_id: String,
}

impl HelpHandler {
    pub fn new(instance_id: String) -> Self {
        Self { instance_id }
    }

    pub async fn help(&self, params: UiHelpParams) -> Result<HelpResponse> {
        tracing::info!(
            "Processing help request for instance '{}'",
            self.instance_id
        );

        let response = match params.tool.as_deref() {
            Some("ui_think") => self.ui_think_help(&params.topic),
            Some("ui_recall") => self.ui_recall_help(&params.topic),
            Some("ui_remember") => self.ui_remember_help(&params.topic),
            Some("ui_start") => self.ui_start_help(&params.topic),
            _ => self.general_help(),
        };

        Ok(response)
    }

    fn general_help(&self) -> HelpResponse {
        HelpResponse {
            overview: "UnifiedIntelligence MCP Server - A Redis-backed thought storage and retrieval system with workflow frameworks and internal thinking modes.\n\nAvailable tools:\n• ui_think - Capture and process thoughts with optional chaining and framework_state\n• ui_recall - Retrieve thoughts by ID or chain ID\n• ui_remember - Conversational memory with assistant synthesis + feedback\n• ui_help - Get help information about the tools".to_string(),
            tools: json!({
                "ui_think": {
                    "description": "Capture and process thoughts with optional chaining support",
                    "purpose": "Store structured thoughts with metadata for later retrieval and analysis",
                    "key_features": [
                        "Sequential thought chaining",
                        "Framework states + internal thinking modes",
                        "Importance and relevance scoring",
                        "Categorization and tagging",
                        "Automatic embeddings generation"
                    ]
                },
                "ui_recall": {
                    "description": "Retrieve thoughts and memories by ID or chain ID",
                    "purpose": "Access previously stored thoughts for context and continuity",
                    "modes": [
                        "thought - Retrieve a single thought by ID",
                        "chain - Retrieve all thoughts in a chain"
                    ]
                },
                "ui_remember": {
                    "description": "Conversational memory: store user query, synthesize response, and record feedback",
                    "purpose": "Single-tool flow with action=\"query\" or action=\"feedback\"; returns next_action contract"
                },
                "ui_start": {
                    "description": "Start a session: summarize previous chain, embed, TTL, set new chain_id",
                    "purpose": "Provides a 10k structured summary for LLM clients and prepares state for the new session"
                },
                "ui_help": {
                    "description": "Get help information about available tools",
                    "purpose": "Learn how to use UnifiedIntelligence tools effectively"
                }
            }),
            examples: json!({
                "basic_thought": {
                    "description": "Store a simple thought",
                    "params": {
                        "thought": "The Redis connection pool should be optimized for concurrent access",
                        "thought_number": 1,
                        "total_thoughts": 1,
                        "next_thought_needed": false
                    }
                },
                "chained_thoughts": {
                    "description": "Create a chain of related thoughts",
                    "params": {
                        "thought": "Starting analysis of system architecture",
                        "thought_number": 1,
                        "total_thoughts": 3,
                        "next_thought_needed": true,
                        "chain_id": "20240129-architecture-review"
                    }
                },
                "framework_state": {
                    "description": "Use a workflow framework state",
                    "params": {
                        "thought": "Why is the API response time increasing?",
                        "thought_number": 1,
                        "total_thoughts": 5,
                        "next_thought_needed": true,
                        "framework_state": "debug",
                        "chain_id": "20240129-performance-analysis"
                    }
                }
            }),
            tips: vec![
                "Use chain_id to link related thoughts together for better context".to_string(),
                "Set framework_state to guide interaction (conversation, debug, build, stuck, review); internal modes (first_principles, ooda, systems, root_cause, swot, socratic) are selected automatically".to_string(),
                "Add importance (1-10) and relevance (1-10) scores for prioritization".to_string(),
                "Use tags and categories to organize thoughts for easier retrieval".to_string(),
                "The thought_number and total_thoughts help track progress in multi-step thinking".to_string(),
            ],
        }
    }

    fn ui_remember_help(&self, topic: &Option<String>) -> HelpResponse {
        let parameters = json!({
            "description": "Conversational memory with two actions: query and feedback",
            "required_params": {
                "action": "'query' (default) or 'feedback'",
                "thought": "User input text (required for query)",
                "chain_id": "Conversation chain (optional for query; required for feedback)"
            },
            "optional_params": {
                "style": "Synthesis style hint (e.g., deep|chronological)",
                "tags": "Array of tags for organization",
                "feedback": "Freeform critique text (required for feedback)",
                "continue_next": "Bool; if true after feedback, suggests another query"
            },
            "flow": "T1 user thought -> T2 synthesized assistant -> T3 feedback/metrics",
            "notes": [
                "If chain_id is omitted on query, server mints remember:UUID",
                "On query, response includes next_action with tool/ui_remember feedback requirements"
            ]
        });

        let examples = json!({
            "query": {
                "description": "Store a user thought and get assistant synthesis + next_action contract",
                "params": {
                    "action": "query",
                    "thought": "Summarize design risks for the frameworks refactor",
                    "tags": ["design", "risk"]
                }
            },
            "feedback": {
                "description": "Attach feedback to the latest assistant synthesis in the chain",
                "params": {
                    "action": "feedback",
                    "chain_id": "remember:…",
                    "feedback": "Good summary; missing rollout checks and runbook links.",
                    "continue_next": true
                }
            },
            "next_action_contract": {
                "description": "Contract returned after a query to guide the next tool call",
                "example": {
                    "next_action": {
                        "tool": "ui_remember",
                        "action": "feedback",
                        "required": ["chain_id", "feedback"],
                        "optional": ["continue_next"]
                    }
                }
            }
        });

        HelpResponse {
            overview: "ui_remember - Conversational memory with assistant synthesis and feedback"
                .to_string(),
            tools: match topic.as_deref() {
                Some("parameters") => parameters,
                Some("examples") => examples,
                _ => json!({
                    "parameters": parameters,
                    "examples": examples
                }),
            },
            examples: json!({}),
            tips: vec![
                "Default action is 'query'; omit chain_id to mint one".to_string(),
                "Use 'feedback' with chain_id to record critique and optionally continue"
                    .to_string(),
                "Honor next_action contract fields to avoid parameter drift".to_string(),
            ],
        }
    }

    fn ui_think_help(&self, topic: &Option<String>) -> HelpResponse {
        let base_info = json!({
            "description": "Capture and process thoughts with optional chaining support",
            "required_params": {
                "thought": "The thought content to process (string)",
                "thought_number": "Current thought number in sequence (integer)",
                "total_thoughts": "Total number of thoughts in sequence (integer)",
                "next_thought_needed": "Whether another thought is needed (boolean)"
            },
            "optional_params": {
                "chain_id": "Optional chain ID to link thoughts together (string)",
                "framework_state": "Workflow framework state (string): 'conversation' (default), 'debug', 'build', 'stuck', 'review'",
                "importance": "Importance score from 1-10 scale (integer)",
                "relevance": "Relevance score from 1-10 scale to current task (integer)",
                "tags": "Tags for categorization (array of strings)",
                "category": "Category: 'technical', 'strategic', 'operational', or 'relationship' (string)"
            }
        });
        let frameworks = json!({
            "frameworks": {
                "conversation": { "default": true, "notes": "read-only; focus on capturing", "modes": ["first_principles","systems","swot"] },
                "debug":        { "modes": ["root_cause","ooda","socratic"] },
                "build":        { "modes": [] },
                "stuck":        { "modes": ["first_principles","socratic","systems","ooda","root_cause"] },
                "review":       { "modes": ["socratic","systems","first_principles"] }
            },
            "thinking_modes": ["first_principles","socratic","systems","ooda","root_cause","swot"]
        });

        let examples = json!({
            "simple_thought": {
                "params": {
                    "thought": "The cache invalidation strategy needs review",
                    "thought_number": 1,
                    "total_thoughts": 1,
                    "next_thought_needed": false
                }
            },
            "framework_example": {
                "params": {
                    "thought": "Database performance is degrading under load",
                    "thought_number": 1,
                    "total_thoughts": 5,
                    "next_thought_needed": true,
                    "framework_state": "debug",
                    "chain_id": "20240129-db-performance",
                    "importance": 8,
                    "relevance": 10,
                    "tags": ["database", "performance", "critical"],
                    "category": "technical"
                }
            },
            "chained_example": {
                "params": {
                    "thought": "Continuing the analysis - found that connection pooling is misconfigured",
                    "thought_number": 2,
                    "total_thoughts": 5,
                    "next_thought_needed": true,
                    "chain_id": "20240129-db-performance",
                    "framework_state": "debug"
                }
            }
        });

        HelpResponse {
            overview: "ui_think - Capture and process thoughts with advanced features for structured thinking".to_string(),
            tools: match topic.as_deref() {
                Some("frameworks") => frameworks,
                Some("parameters") => base_info,
                Some("examples") => examples,
                _ => json!({
                    "parameters": base_info,
                    "frameworks": frameworks,
                    "examples": examples
                })
            },
            examples: json!({}),
            tips: vec![
                "Always set thought_number and total_thoughts accurately for proper sequencing".to_string(),
                "Use chain_id consistently to link related thoughts".to_string(),
                "Choose frameworks that match your thinking needs".to_string(),
                "Higher importance scores (8-10) indicate critical insights".to_string(),
                "Tags should be lowercase and descriptive".to_string(),
                "When framework_state='stuck', we persist a per-chain StuckTracker in Redis to rotate thinking modes; include chain_id to enable persistence".to_string(),
            ],
        }
    }

    fn ui_recall_help(&self, topic: &Option<String>) -> HelpResponse {
        let base_info = json!({
            "description": "Retrieve thoughts and memories by ID or chain ID",
            "required_params": {
                "mode": "The recall mode: 'thought' or 'chain' (string)",
                "id": "The thought ID or chain ID to retrieve (string)"
            },
            "modes": {
                "thought": {
                    "description": "Retrieve a single thought by its unique ID",
                    "returns": "A single thought record with all metadata"
                },
                "chain": {
                    "description": "Retrieve all thoughts in a chain",
                    "returns": "Array of thoughts ordered by thought_number"
                }
            }
        });

        let examples = json!({
            "recall_single_thought": {
                "description": "Retrieve a specific thought",
                "params": {
                    "mode": "thought",
                    "id": "96331831-0fa7-4da0-8445-7d3b0a0fdf44"
                }
            },
            "recall_chain": {
                "description": "Retrieve all thoughts in a chain",
                "params": {
                    "mode": "chain",
                    "id": "20240129-architecture-review"
                }
            }
        });

        HelpResponse {
            overview: "ui_recall - Retrieve previously stored thoughts for context and analysis"
                .to_string(),
            tools: match topic.as_deref() {
                Some("parameters") => base_info,
                Some("examples") => examples,
                _ => json!({
                    "parameters": base_info,
                    "examples": examples
                }),
            },
            examples: json!({}),
            tips: vec![
                "Use 'thought' mode when you have a specific thought ID".to_string(),
                "Use 'chain' mode to retrieve entire thought sequences".to_string(),
                "Chain IDs typically follow format: YYYYMMDD-topic-description".to_string(),
                "Recalled thoughts include all metadata (timestamps, scores, tags)".to_string(),
            ],
        }
    }

    fn ui_start_help(&self, topic: &Option<String>) -> HelpResponse {
        let parameters = json!({
            "description": "Start a new session by generating a 10k structured summary from the previous chain and returning it with a new chain_id.",
            "required_params": {
                "user": "User entity name or ID in the Knowledge Graph"
            },
            "optional_params": {
                "model": "Model preference (fast|deep or explicit)",
                "scope": "Knowledge scope: federation|personal (default: federation)",
                "summary_tokens": "Target token cap for the summary (default 10000)"
            },
            "output": {
                "new_chain_id": "Chain ID for the new/current session",
                "summary_key": "Redis key for the cached summary (1h TTL)",
                "summary_text": "Structured text suitable for LLM clients"
            }
        });

        let examples = json!({
            "start_default": {
                "params": {"user": "samuel"}
            }
        });

        HelpResponse {
            overview: "ui_start - Session bootstrap: produce a 10k structured summary for LLM clients and set up new session state".to_string(),
            tools: match topic.as_deref() {
                Some("parameters") => parameters,
                Some("examples") => examples,
                _ => json!({
                    "parameters": parameters,
                    "examples": examples
                }),
            },
            examples: json!({}),
            tips: vec![
                "Summary JSON is stored with 1h TTL and chunk embeddings for retrieval".to_string(),
                "KG attributes maintained: current_session_chain_id, session_history, summary_keys".to_string(),
                "Initial MVP returns only the 10k summary; expanded multi-summary synthesis can be added later".to_string(),
            ],
        }
    }
}

/// Trait for help-related operations
pub trait HelpHandlerTrait {
    /// Handle ui_help tool
    async fn ui_help(&self, params: UiHelpParams) -> Result<HelpResponse>;
}

impl<R: ThoughtRepository + KnowledgeRepository> HelpHandlerTrait for super::ToolHandlers<R> {
    async fn ui_help(&self, params: UiHelpParams) -> Result<HelpResponse> {
        self.help.help(params).await
    }
}
