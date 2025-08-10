use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UiRememberParams {
    pub thought: String,
    pub thought_number: i32,
    pub total_thoughts: i32,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[allow(dead_code)]
    pub next_thought_needed: bool,

    // Optional guidance the voice may ignore
    #[serde(default)]
    #[allow(dead_code)]
    pub search_type: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub top_k: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub similarity_threshold: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub token_cap: Option<i32>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub temporal: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UiRememberResult {
    pub status: String,
    pub thought1_id: String,
    pub thought2_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought3_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_total_tokens: Option<i32>,
}
