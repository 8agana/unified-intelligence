use serde::{Deserialize, Serialize, Deserializer};
use chrono::Utc;

/// Flexible integer deserializer to handle string, float, or int inputs from different MCP clients
fn deserialize_flexible_int<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleInt {
        Int(i32),
        Float(f64),
        String(String),
    }
    
    let value = FlexibleInt::deserialize(deserializer)?;
    match value {
        FlexibleInt::Int(i) => Ok(Some(i)),
        FlexibleInt::Float(f) => Ok(Some(f as i32)),
        FlexibleInt::String(s) => {
            s.parse::<i32>().map(Some).map_err(serde::de::Error::custom)
        }
    }
}

/// Flexible integer deserializer for required fields
fn deserialize_flexible_int_required<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleInt {
        Int(i32),
        Float(f64),
        String(String),
    }
    
    let value = FlexibleInt::deserialize(deserializer)?;
    match value {
        FlexibleInt::Int(i) => Ok(i),
        FlexibleInt::Float(f) => Ok(f as i32),
        FlexibleInt::String(s) => {
            s.parse::<i32>().map_err(serde::de::Error::custom)
        }
    }
}

/// Parameters for the ui_think tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiThinkParams {
    #[schemars(description = "The thought content to process")]
    pub thought: String,
    
    #[schemars(description = "Current thought number in sequence")]
    #[serde(deserialize_with = "deserialize_flexible_int_required")]
    pub thought_number: i32,
    
    #[schemars(description = "Total number of thoughts in sequence")]
    #[serde(deserialize_with = "deserialize_flexible_int_required")]
    pub total_thoughts: i32,
    
    #[schemars(description = "Whether another thought is needed")]
    pub next_thought_needed: bool,
    
    #[schemars(description = "Optional chain ID to link thoughts together")]
    pub chain_id: Option<String>,
    
    #[schemars(description = "Optional thinking framework: 'ooda', 'socratic', 'first_principles', 'systems', 'root_cause', 'swot', 'sequential', 'remember', 'deepremember'")]
    pub framework: Option<String>,
    
    // NEW METADATA FIELDS FOR FEEDBACK LOOP SYSTEM
    #[schemars(description = "Importance score from 1-10 scale")]
    #[serde(deserialize_with = "deserialize_flexible_int")]
    pub importance: Option<i32>,
    
    #[schemars(description = "Relevance score from 1-10 scale (to current task)")]
    #[serde(deserialize_with = "deserialize_flexible_int")]
    pub relevance: Option<i32>,
    
    #[schemars(description = "Tags for categorization (e.g., ['architecture', 'redis', 'critical'])")]
    pub tags: Option<Vec<String>>,
    
    #[schemars(description = "Category: 'technical', 'strategic', 'operational', or 'relationship'")]
    pub category: Option<String>,
}


/// Core thought record structure stored in Redis
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThoughtRecord {
    pub id: String,
    pub instance: String,
    pub thought: String,
    pub content: String, // Alias for thought for compatibility
    pub thought_number: i32,
    pub total_thoughts: i32,
    pub timestamp: String,
    pub chain_id: Option<String>,
    pub next_thought_needed: bool,
    pub framework: Option<String>,
    pub importance: Option<i32>,
    pub relevance: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
}

impl ThoughtRecord {
    /// Create a new thought record with generated ID and timestamp
    pub fn new(
        instance: String,
        thought: String,
        thought_number: i32,
        total_thoughts: i32,
        chain_id: Option<String>,
        next_thought_needed: bool,
        framework: Option<String>,
        importance: Option<i32>,
        relevance: Option<i32>,
        tags: Option<Vec<String>>,
        category: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            instance,
            thought: thought.clone(),
            content: thought, // Duplicate for compatibility
            thought_number,
            total_thoughts,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            chain_id,
            next_thought_needed,
            framework,
            importance,
            relevance,
            tags,
            category,
            }
    }
}


/// Response from ui_think tool
#[derive(Debug, Serialize)]
pub struct ThinkResponse {
    pub status: String,
    pub thought_id: String,
    pub next_thought_needed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_generated_thought: Option<ThoughtRecord>,
}

/// Chain metadata stored in Redis
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainMetadata {
    pub chain_id: String,
    pub created_at: String,
    pub thought_count: i32,
    pub instance: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Thought {
    pub id: uuid::Uuid,
    pub content: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub instance_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub importance: i32,
    pub relevance: i32,
    pub semantic_score: Option<f32>,
    pub temporal_score: Option<f32>,
    pub usage_score: Option<f32>,
    pub combined_score: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TemporalFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub relative_timeframe: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct QueryIntent {
    pub original_query: String,
    pub temporal_filter: Option<TemporalFilter>,
    pub synthesis_style: Option<String>,
}

// Groq chat message format
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Groq API request format
#[derive(Debug, Serialize, Clone)]
pub struct GroqRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
}

// Groq API response format
#[derive(Debug, Deserialize)]
pub struct GroqResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
}

// RecallMode enum and UiRecallParams moved to handlers/recall.rs with string-based mode
