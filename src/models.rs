use chrono::Utc;
use serde::{Deserialize, Deserializer, Serialize};

fn default_importance() -> Option<i32> {
    Some(5)
}

fn default_relevance() -> Option<i32> {
    Some(5)
}

fn default_category() -> Option<String> {
    Some("general".to_string())
}

fn default_tags() -> Option<Vec<String>> {
    Some(vec![])
}

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
        FlexibleInt::String(s) => s.parse::<i32>().map(Some).map_err(serde::de::Error::custom),
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
        FlexibleInt::String(s) => s.parse::<i32>().map_err(serde::de::Error::custom),
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

    #[schemars(
        description = "Optional thinking framework: 'ooda', 'socratic', 'first_principles', 'systems', 'root_cause', 'swot', 'sequential', 'remember', 'deepremember'"
    )]
    pub framework: Option<String>,

    // NEW METADATA FIELDS FOR FEEDBACK LOOP SYSTEM
    #[schemars(description = "Importance score from 1-10 scale")]
    #[serde(
        deserialize_with = "deserialize_flexible_int",
        default = "default_importance"
    )]
    pub importance: Option<i32>,

    #[schemars(description = "Relevance score from 1-10 scale (to current task)")]
    #[serde(
        deserialize_with = "deserialize_flexible_int",
        default = "default_relevance"
    )]
    pub relevance: Option<i32>,

    #[schemars(
        description = "Tags for categorization (e.g., ['architecture', 'redis', 'critical'])"
    )]
    #[serde(default = "default_tags")]
    pub tags: Option<Vec<String>>,

    #[schemars(
        description = "Category: 'technical', 'strategic', 'operational', or 'relationship'"
    )]
    #[serde(default = "default_category")]
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
    #[allow(clippy::too_many_arguments)]
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

// ========== KNOWLEDGE GRAPH MODELS ==========

/// Scope for knowledge graph operations
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeScope {
    Federation,  // Default for work-related entities
    Personal,    // Instance-specific context
}

impl Default for KnowledgeScope {
    fn default() -> Self {
        Self::Federation
    }
}

impl std::fmt::Display for KnowledgeScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Federation => write!(f, "Federation"),
            Self::Personal => write!(f, "Personal"),
        }
    }
}

/// Entity types in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Issue,
    Person,
    System,
    Concept,
    Tool,
    Framework,
    Custom(String),
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Issue => write!(f, "issue"),
            Self::Person => write!(f, "person"),
            Self::System => write!(f, "system"),
            Self::Concept => write!(f, "concept"),
            Self::Tool => write!(f, "tool"),
            Self::Framework => write!(f, "framework"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub entity_type: EntityType,
    pub scope: KnowledgeScope,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by: String,
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
    pub tags: Vec<String>,
    pub thought_ids: Vec<String>,
    pub embedding: Option<Vec<f32>>,
    pub metadata: NodeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub auto_extracted: bool,
    pub extraction_source: Option<String>,
    pub extraction_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRelation {
    pub id: String,
    pub from_entity_id: String,
    pub to_entity_id: String,
    pub relationship_type: String,
    pub scope: KnowledgeScope,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: String,
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
    pub metadata: RelationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationMetadata {
    pub bidirectional: bool,
    pub weight: f32,
}

/// Parameters for ui_knowledge tool - flattened structure for CC compatibility
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UiKnowledgeParams {
    #[schemars(description = "Action to perform: create, search, set_active, get_entity, create_relation, get_relations, update_entity, delete_entity")]
    pub action: String,
    
    // Create entity fields
    #[schemars(description = "Name/identifier for the entity (create)")]
    pub name: Option<String>,
    #[schemars(description = "Human-readable display name (create/update)")]
    pub display_name: Option<String>,
    #[schemars(description = "Type of entity: person, system, tool, concept, framework, issue, custom (create/search)")]
    pub entity_type: Option<String>,
    
    // Common fields
    #[schemars(description = "Scope: 'federation' or 'personal' (all operations)")]
    pub scope: Option<String>,
    #[schemars(description = "Additional attributes as JSON object (create/update/create_relation)")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
    #[schemars(description = "Tags for categorization (create/update)")]
    pub tags: Option<Vec<String>>,
    
    // Search fields
    #[schemars(description = "Search query (search)")]
    pub query: Option<String>,
    #[schemars(description = "Maximum results to return (search)")]
    pub limit: Option<usize>,
    
    // Entity ID fields
    #[schemars(description = "Entity ID (set_active/get_entity/get_relations/update_entity/delete_entity)")]
    pub entity_id: Option<String>,
    
    // Relation fields
    #[schemars(description = "Source entity ID (create_relation)")]
    pub from_entity_id: Option<String>,
    #[schemars(description = "Target entity ID (create_relation)")]
    pub to_entity_id: Option<String>,
    #[schemars(description = "Type of relationship (create_relation)")]
    pub relationship_type: Option<String>,
    #[schemars(description = "Whether the relationship is bidirectional (create_relation)")]
    pub bidirectional: Option<bool>,
    #[schemars(description = "Weight of the relationship 0.0-1.0 (create_relation)")]
    pub weight: Option<f32>,
}

// Keep the enum for internal use (not exposed in schema)
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UiKnowledgeOperation {
    Create(CreateEntityParams),
    Search(SearchParams),
    SetActive(SetActiveParams),
    GetEntity(GetEntityParams),
    CreateRelation(CreateRelationParams),
    GetRelations(GetRelationsParams),
    UpdateEntity(UpdateEntityParams),
    DeleteEntity(DeleteEntityParams),
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateEntityParams {
    #[schemars(description = "Name/identifier for the entity")]
    pub name: String,
    
    #[schemars(description = "Human-readable display name")]
    pub display_name: Option<String>,
    
    #[schemars(description = "Type of entity")]
    pub entity_type: EntityType,
    
    #[schemars(description = "Scope: 'federation' (default) or 'personal'")]
    #[serde(default)]
    pub scope: KnowledgeScope,
    
    #[schemars(description = "Additional attributes as JSON object")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
    
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query (name, tag, or attribute)")]
    pub query: String,
    
    #[schemars(description = "Scope to search in")]
    #[serde(default)]
    pub scope: KnowledgeScope,
    
    #[schemars(description = "Filter by entity type")]
    pub entity_type: Option<EntityType>,
    
    #[schemars(description = "Maximum results to return")]
    #[serde(default = "default_kg_limit")]
    pub limit: usize,
}

fn default_kg_limit() -> usize { 10 }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetActiveParams {
    #[schemars(description = "Entity ID to set as active context")]
    pub entity_id: String,
    
    #[schemars(description = "Scope of the entity")]
    #[serde(default)]
    pub scope: KnowledgeScope,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEntityParams {
    #[schemars(description = "Entity ID to retrieve")]
    pub entity_id: String,
    
    #[schemars(description = "Scope of the entity")]
    #[serde(default)]
    pub scope: KnowledgeScope,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRelationParams {
    #[schemars(description = "Source entity ID")]
    pub from_entity_id: String,
    
    #[schemars(description = "Target entity ID")]
    pub to_entity_id: String,
    
    #[schemars(description = "Type of relationship")]
    pub relationship_type: String,
    
    #[schemars(description = "Scope of the relationship")]
    #[serde(default)]
    pub scope: KnowledgeScope,
    
    #[schemars(description = "Additional attributes for the relationship")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
    
    #[schemars(description = "Whether the relationship is bidirectional")]
    #[serde(default)]
    pub bidirectional: bool,
    
    #[schemars(description = "Weight of the relationship (0.0-1.0)")]
    #[serde(default = "default_kg_weight")]
    pub weight: f32,
}

fn default_kg_weight() -> f32 { 1.0 }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRelationsParams {
    #[schemars(description = "Entity ID to get relations for")]
    pub entity_id: String,
    
    #[schemars(description = "Scope of the entity")]
    #[serde(default)]
    pub scope: KnowledgeScope,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateEntityParams {
    #[schemars(description = "Entity ID to update")]
    pub entity_id: String,
    
    #[schemars(description = "Scope of the entity")]
    #[serde(default)]
    pub scope: KnowledgeScope,
    
    #[schemars(description = "Updated display name")]
    pub display_name: Option<String>,
    
    #[schemars(description = "Updated attributes")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,
    
    #[schemars(description = "Updated tags")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteEntityParams {
    #[schemars(description = "Entity ID to delete")]
    pub entity_id: String,
    
    #[schemars(description = "Scope of the entity")]
    #[serde(default)]
    pub scope: KnowledgeScope,
}

/// Response from knowledge operations
#[derive(Debug, Serialize)]
pub struct KnowledgeResponse {
    pub status: String,
    pub entity_id: Option<String>,
    pub entities: Option<Vec<KnowledgeNode>>,
    pub relations: Option<Vec<KnowledgeRelation>>,
    pub message: Option<String>,
}

impl KnowledgeScope {
    /// Determine appropriate scope based on context
    pub fn from_context(entity_type: &EntityType, instance: &str) -> Self {
        match entity_type {
            EntityType::Issue | EntityType::System | EntityType::Tool => {
                // Work-related entities default to Federation
                Self::Federation
            }
            EntityType::Person => {
                // Check if it's a known team member
                if ["Sam", "CC", "DT", "Gem"].contains(&instance) {
                    Self::Federation
                } else {
                    Self::Personal
                }
            }
            EntityType::Concept | EntityType::Framework => {
                // Technical concepts are Federation, personal notes are Personal
                Self::Federation
            }
            EntityType::Custom(_) => {
                // Custom entities default to Personal
                Self::Personal
            }
        }
    }
}
