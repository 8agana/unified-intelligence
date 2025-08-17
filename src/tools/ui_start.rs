use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::models::KnowledgeScope;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UiStartParams {
    /// User entity name or ID in the Knowledge Graph
    pub user: String,
    /// Optional model preference (e.g., fast|deep or explicit)
    #[serde(default)]
    #[allow(dead_code)]
    pub model: Option<String>,
    /// Scope for the user entity (defaults to Federation)
    #[serde(default)]
    pub scope: Option<KnowledgeScope>,
    /// Max tokens for the structured session summary (default ~10000)
    #[serde(default)]
    #[allow(dead_code)]
    pub summary_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiStartResult {
    pub status: String,
    pub new_chain_id: String,
    pub summary_key: String,
    pub summary_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_total_tokens: Option<i32>,
}
