use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_empty_string() -> String {
    String::new()
}

fn default_thought_number() -> i32 {
    1
}

fn default_total_thoughts() -> i32 {
    1
}

fn default_next_thought_needed() -> bool {
    false
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UiRememberParams {
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default = "default_empty_string")]
    pub thought: String,
    #[serde(default = "default_thought_number")]
    pub thought_number: i32,
    #[serde(default = "default_total_thoughts")]
    pub total_thoughts: i32,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default = "default_next_thought_needed")]
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

    /// Optional LLM feedback on the synthesis/retrieval (when action="feedback")
    #[serde(default)]
    pub feedback: Option<String>,
    /// Optional decision to continue after feedback
    #[serde(default)]
    pub continue_next: Option<bool>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_text_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_embedding_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<NextAction>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NextAction {
    pub tool: String,
    pub action: String,
    pub required: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub optional: Vec<String>,
}
