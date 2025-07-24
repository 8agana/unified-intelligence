use crate::error::Result;
use crate::models::{Identity, ThoughtRecord};
use crate::repository_traits::{IdentityRepository, SearchRepository, ThoughtRepository};
use crate::service::UnifiedIntelligenceService;
use crate::visual::VisualFeedback;
use anyhow::anyhow;
use rmcp::{HandlerError, HandlerResult, RequestParams, Tool, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct RecallRequest {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    include_chains: bool,
}

fn default_limit() -> usize {
    10
}

#[derive(Debug, Serialize)]
struct RecallResponse {
    query: String,
    thoughts_found: usize,
    thoughts: Vec<ThoughtSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chains: Option<Vec<ChainSummary>>,
    context_summary: String,
}

#[derive(Debug, Serialize)]
struct ThoughtSummary {
    id: String,
    content: String,
    timestamp: String,
    importance: Option<f32>,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    framework: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChainSummary {
    chain_id: String,
    thought_count: usize,
    first_thought: String,
    last_thought: String,
}

pub struct RecallHandler {
    service: Arc<UnifiedIntelligenceService>,
}

impl RecallHandler {
    pub fn new(service: Arc<UnifiedIntelligenceService>) -> Self {
        Self { service }
    }

    pub fn tool() -> Tool {
        Tool {
            name: "ui_recall".to_string(),
            description: Some("Search and recall relevant thoughts and memories from UnifiedIntelligence. This tool performs semantic search across all stored thoughts and can optionally include chain analysis.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find relevant thoughts"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)",
                        "minimum": 1,
                        "maximum": 100
                    },
                    "include_chains": {
                        "type": "boolean",
                        "description": "Whether to include chain analysis in results (default: false)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn process_recall(&self, request: RecallRequest) -> Result<RecallResponse> {
        let visual = VisualFeedback::new();
        visual.thinking(&format!("Recalling thoughts for query: '{}'", request.query));

        // Perform search
        let search_results = self
            .service
            .search_repository()
            .search_thoughts(&request.query, request.limit)
            .await?;

        if search_results.is_empty() {
            visual.error("No thoughts found matching the query");
            return Ok(RecallResponse {
                query: request.query,
                thoughts_found: 0,
                thoughts: vec![],
                chains: None,
                context_summary: "No relevant thoughts found for this query.".to_string(),
            });
        }

        visual.success(&format!("Found {} relevant thoughts", search_results.len()));

        // Convert to summaries
        let mut thought_summaries: Vec<ThoughtSummary> = search_results
            .iter()
            .map(|record| ThoughtSummary {
                id: record.id.clone(),
                content: record.content.clone(),
                timestamp: record.timestamp.clone(),
                importance: record.metadata.as_ref().and_then(|m| m.importance),
                tags: record
                    .metadata
                    .as_ref()
                    .and_then(|m| m.tags.clone())
                    .unwrap_or_default(),
                framework: record.metadata.as_ref().and_then(|m| m.framework.clone()),
            })
            .collect();

        // Handle chain analysis if requested
        let mut chains = None;
        if request.include_chains {
            visual.thinking("Analyzing thought chains...");
            let mut chain_map: std::collections::HashMap<String, Vec<&ThoughtRecord>> = 
                std::collections::HashMap::new();

            for record in &search_results {
                if let Some(chain_id) = &record.chain_id {
                    chain_map.entry(chain_id.clone()).or_default().push(record);
                }
            }

            if !chain_map.is_empty() {
                let chain_summaries: Vec<ChainSummary> = chain_map
                    .into_iter()
                    .map(|(chain_id, thoughts)| ChainSummary {
                        chain_id,
                        thought_count: thoughts.len(),
                        first_thought: thoughts
                            .first()
                            .map(|t| t.content.chars().take(100).collect())
                            .unwrap_or_default(),
                        last_thought: thoughts
                            .last()
                            .map(|t| t.content.chars().take(100).collect())
                            .unwrap_or_default(),
                    })
                    .collect();
                chains = Some(chain_summaries);
            }
        }

        // Generate context summary
        let context_summary = self.generate_context_summary(&search_results);

        Ok(RecallResponse {
            query: request.query,
            thoughts_found: thought_summaries.len(),
            thoughts: thought_summaries,
            chains,
            context_summary,
        })
    }

    fn generate_context_summary(&self, thoughts: &[ThoughtRecord]) -> String {
        // Analyze the thoughts to provide a high-level summary
        let total = thoughts.len();
        let with_chains = thoughts.iter().filter(|t| t.chain_id.is_some()).count();
        let unique_frameworks: std::collections::HashSet<_> = thoughts
            .iter()
            .filter_map(|t| t.metadata.as_ref()?.framework.as_ref())
            .collect();

        let mut summary = format!("Found {} relevant thoughts. ", total);
        
        if with_chains > 0 {
            summary.push_str(&format!("{} are part of thought chains. ", with_chains));
        }
        
        if !unique_frameworks.is_empty() {
            summary.push_str(&format!(
                "Frameworks used: {}. ",
                unique_frameworks
                    .into_iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Add time range if available
        if let (Some(first), Some(last)) = (thoughts.first(), thoughts.last()) {
            if first.timestamp != last.timestamp {
                summary.push_str(&format!(
                    "Time span: {} to {}.",
                    first.timestamp.split('T').next().unwrap_or(&first.timestamp),
                    last.timestamp.split('T').next().unwrap_or(&last.timestamp)
                ));
            }
        }

        summary
    }
}

#[async_trait::async_trait]
impl ToolHandler for RecallHandler {
    async fn run(&self, params: RequestParams) -> HandlerResult {
        let request: RecallRequest = serde_json::from_value(params.arguments)
            .map_err(|e| HandlerError::InvalidParams(e.to_string()))?;

        match self.process_recall(request).await {
            Ok(response) => {
                let content = vec![rmcp::types::TextContent {
                    text: serde_json::to_string_pretty(&response)
                        .unwrap_or_else(|_| "Failed to serialize response".to_string()),
                }
                .into()];
                Ok(content)
            }
            Err(e) => {
                let visual = VisualFeedback::new();
                visual.error(&format!("Recall failed: {}", e));
                Err(HandlerError::ToolError(e.to_string()))
            }
        }
    }
}