use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::repository::ThoughtRepository;

/// Parameters for the ui_help tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiHelpParams {
    #[schemars(description = "Optional specific tool to get help for ('ui_think', 'ui_recall', or leave empty for general help)")]
    pub tool: Option<String>,
    
    #[schemars(description = "Optional specific topic ('frameworks', 'parameters', 'examples', or leave empty for all)")]
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
        tracing::info!("Processing help request for instance '{}'", self.instance_id);
        
        let response = match params.tool.as_deref() {
            Some("ui_think") => self.ui_think_help(&params.topic),
            Some("ui_recall") => self.ui_recall_help(&params.topic),
            _ => self.general_help(),
        };
        
        Ok(response)
    }
    
    fn general_help(&self) -> HelpResponse {
        HelpResponse {
            overview: "UnifiedIntelligence MCP Server - A Redis-backed thought storage and retrieval system with thinking framework support.\n\nAvailable tools:\n• ui_think - Capture and process thoughts with optional chaining and framework support\n• ui_recall - Retrieve thoughts by ID or chain ID\n• ui_help - Get help information about the tools".to_string(),
            
            tools: json!({
                "ui_think": {
                    "description": "Capture and process thoughts with optional chaining support",
                    "purpose": "Store structured thoughts with metadata for later retrieval and analysis",
                    "key_features": [
                        "Sequential thought chaining",
                        "Multiple thinking frameworks",
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
                "framework_thinking": {
                    "description": "Use a thinking framework",
                    "params": {
                        "thought": "Why is the API response time increasing?",
                        "thought_number": 1,
                        "total_thoughts": 5,
                        "next_thought_needed": true,
                        "framework": "root_cause",
                        "chain_id": "20240129-performance-analysis"
                    }
                }
            }),
            
            tips: vec![
                "Use chain_id to link related thoughts together for better context".to_string(),
                "Apply thinking frameworks to structure your analysis (ooda, socratic, first_principles, systems, root_cause, swot)".to_string(),
                "Add importance (1-10) and relevance (1-10) scores for prioritization".to_string(),
                "Use tags and categories to organize thoughts for easier retrieval".to_string(),
                "The thought_number and total_thoughts help track progress in multi-step thinking".to_string(),
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
                "framework": "Optional thinking framework (string): 'ooda', 'socratic', 'first_principles', 'systems', 'root_cause', 'swot'",
                "importance": "Importance score from 1-10 scale (integer)",
                "relevance": "Relevance score from 1-10 scale to current task (integer)",
                "tags": "Tags for categorization (array of strings)",
                "category": "Category: 'technical', 'strategic', 'operational', or 'relationship' (string)"
            }
        });
        
        let frameworks = json!({
            "available_frameworks": {
                "ooda": {
                    "name": "OODA Loop",
                    "description": "Observe, Orient, Decide, Act - for rapid decision-making",
                    "use_when": "Need to make quick decisions or respond to changing situations"
                },
                "socratic": {
                    "name": "Socratic Method",
                    "description": "Question-based exploration to uncover assumptions",
                    "use_when": "Want to deeply understand a concept or challenge assumptions"
                },
                "first_principles": {
                    "name": "First Principles",
                    "description": "Break down to fundamental truths and build up",
                    "use_when": "Need to solve complex problems or innovate"
                },
                "systems": {
                    "name": "Systems Thinking",
                    "description": "Analyze interconnections and feedback loops",
                    "use_when": "Dealing with complex systems or organizational issues"
                },
                "root_cause": {
                    "name": "Root Cause Analysis",
                    "description": "5 Whys and fishbone analysis for problem solving",
                    "use_when": "Investigating problems or failures"
                },
                "swot": {
                    "name": "SWOT Analysis",
                    "description": "Strengths, Weaknesses, Opportunities, Threats",
                    "use_when": "Strategic planning or evaluating options"
                }
            }
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
                    "framework": "root_cause",
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
                    "framework": "root_cause"
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
            overview: "ui_recall - Retrieve previously stored thoughts for context and analysis".to_string(),
            tools: match topic.as_deref() {
                Some("parameters") => base_info,
                Some("examples") => examples,
                _ => json!({
                    "parameters": base_info,
                    "examples": examples
                })
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
}

/// Trait for help-related operations
pub trait HelpHandlerTrait {
    /// Handle ui_help tool
    async fn ui_help(&self, params: UiHelpParams) -> Result<HelpResponse>;
}

impl<R: ThoughtRepository> HelpHandlerTrait for super::ToolHandlers<R> {
    async fn ui_help(&self, params: UiHelpParams) -> Result<HelpResponse> {
        self.help.help(params).await
    }
}