use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Parameters for the ui_think tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiThinkParams {
    #[schemars(description = "The thought content to process")]
    pub thought: String,
    
    #[schemars(description = "Current thought number in sequence")]
    pub thought_number: i32,
    
    #[schemars(description = "Total number of thoughts in sequence")]
    pub total_thoughts: i32,
    
    #[schemars(description = "Whether another thought is needed")]
    pub next_thought_needed: bool,
    
    #[schemars(description = "Optional chain ID to link thoughts together")]
    pub chain_id: Option<String>,
    
    #[schemars(description = "Optional thinking framework: 'ooda', 'socratic', 'first_principles', 'systems', 'root_cause', 'swot'")]
    pub framework: Option<String>,
    
    // NEW METADATA FIELDS FOR FEEDBACK LOOP SYSTEM
    #[schemars(description = "Importance score from 1-10 scale")]
    pub importance: Option<i32>,
    
    #[schemars(description = "Relevance score from 1-10 scale (to current task)")]
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
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            instance,
            thought: thought.clone(),
            content: thought, // Duplicate for compatibility
            thought_number,
            total_thoughts,
            timestamp: Utc::now().to_rfc3339(),
            chain_id,
            next_thought_needed,
            }
    }
}

/// Metadata for thoughts stored separately in Redis for feedback loop system
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThoughtMetadata {
    pub thought_id: String,
    pub instance: String,
    pub importance: Option<i32>,
    pub relevance: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub category: Option<String>,
    pub created_at: String,
}


impl ThoughtMetadata {
    pub fn new(
        thought_id: String,
        instance: String,
        importance: Option<i32>,
        relevance: Option<i32>,
        tags: Option<Vec<String>>,
        category: Option<String>,
    ) -> Self {
        Self {
            thought_id,
            instance,
            importance,
            relevance,
            tags,
            category,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Response from ui_think tool
#[derive(Debug, Serialize)]
pub struct ThinkResponse {
    pub status: String,
    pub thought_id: String,
    pub next_thought_needed: bool,
}

/// Chain metadata stored in Redis
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainMetadata {
    pub chain_id: String,
    pub created_at: String,
    pub thought_count: i32,
    pub instance: String,
}

// RecallMode enum and UiRecallParams moved to handlers/recall.rs with string-based mode