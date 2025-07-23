use serde_json::json;
use tracing;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{
    UiSamParams, SamResponse, SamOperation, Identity,
    OperationHelp, CategoryHelp, FieldTypeHelp, ExampleUsage
};
use crate::repository::ThoughtRepository;

/// Trait for Sam-related operations
pub trait SamHandler {
    /// Handle ui_sam tool
    async fn ui_sam(&self, params: UiSamParams) -> Result<SamResponse>;
}

impl<R: ThoughtRepository> SamHandler for super::ToolHandlers<R> {
    /// Handle ui_sam tool
    async fn ui_sam(&self, params: UiSamParams) -> Result<SamResponse> {
        let operation = params.operation.unwrap_or(SamOperation::View);
        
        tracing::info!(
            "Sam operation '{:?}' - category: {:?}, field: {:?}",
            operation, params.category, params.field
        );
        
        match operation {
            SamOperation::View => {
                // Get Sam's context
                let context = self.repository.get_sam_context().await?;
                
                // Get Sam's identity
                let identity = self.get_or_create_sam_identity().await?;
                
                Ok(SamResponse::View {
                    context,
                    identity,
                    available_categories: vec![
                        "context".to_string(), "core_info".to_string(), "communication".to_string(), 
                        "relationships".to_string(), "work_preferences".to_string(), "behavioral_patterns".to_string(), 
                        "technical_profile".to_string(), "context_awareness".to_string(), "memory_preferences".to_string()
                    ],
                })
            }
            
            SamOperation::Add => {
                let category = params.category.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "category".to_string(),
                    reason: "category required for add operation".to_string(),
                })?;
                let field = params.field.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "field".to_string(),
                    reason: "field required for add operation".to_string(),
                })?;
                let value = params.value.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "value".to_string(),
                    reason: "value required for add operation".to_string(),
                })?;
                
                if category == "context" {
                    self.add_to_sam_context(&field, value).await?;
                } else {
                    self.add_to_sam_identity(&category, &field, value).await?;
                }
                
                Ok(SamResponse::Updated { 
                    operation: "add".to_string(),
                    category, 
                    field: Some(field),
                    success: true,
                })
            }
            
            SamOperation::Modify => {
                let category = params.category.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "category".to_string(),
                    reason: "category required for modify operation".to_string(),
                })?;
                let field = params.field.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "field".to_string(),
                    reason: "field required for modify operation".to_string(),
                })?;
                let value = params.value.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "value".to_string(),
                    reason: "value required for modify operation".to_string(),
                })?;
                
                if category == "context" {
                    self.modify_sam_context(&field, value).await?;
                } else {
                    self.modify_sam_identity(&category, &field, value).await?;
                }
                
                Ok(SamResponse::Updated { 
                    operation: "modify".to_string(),
                    category,
                    field: Some(field),
                    success: true,
                })
            }
            
            SamOperation::Delete => {
                let category = params.category.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "category".to_string(),
                    reason: "category required for delete operation".to_string(),
                })?;
                let field = params.field.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "field".to_string(),
                    reason: "field required for delete operation".to_string(),
                })?;
                
                if category == "context" {
                    self.delete_from_sam_context(&field, params.value).await?;
                } else {
                    self.delete_from_sam_identity(&category, &field, params.value).await?;
                }
                
                Ok(SamResponse::Updated { 
                    operation: "delete".to_string(),
                    category,
                    field: Some(field),
                    success: true,
                })
            }
            
            SamOperation::Help => {
                Ok(self.generate_sam_help_response())
            }
        }
    }
}

// Helper methods implementation for ToolHandlers
impl<R: ThoughtRepository> super::ToolHandlers<R> {
    /// Get or create Sam's identity
    pub(crate) async fn get_or_create_sam_identity(&self) -> Result<Identity> {
        let identity_key = "Sam:Identity";
        
        // Try to get existing identity
        if let Some(identity) = self.repository.get_identity(identity_key).await? {
            Ok(identity)
        } else {
            // Create default identity for Sam
            let identity = self.create_default_sam_identity();
            self.repository.save_identity(identity_key, &identity).await?;
            Ok(identity)
        }
    }
    
    /// Create default Sam identity
    fn create_default_sam_identity(&self) -> Identity {
        Identity {
            core_info: crate::models::CoreInfo {
                name: "Sam".to_string(),
                instance_id: "Human".to_string(),
                instance_type: "Human User".to_string(),
                primary_purpose: "AI system development and personal growth".to_string(),
                core_values: vec![
                    "innovation".to_string(), 
                    "persistence".to_string(), 
                    "humor".to_string(),
                    "growth".to_string()
                ],
            },
            communication: crate::models::CommunicationStyle {
                tone: "direct".to_string(),
                verbosity: "adaptive".to_string(),
                humor_level: 0.9,
                directness: 0.95,
                formality: "informal".to_string(),
            },
            relationships: std::collections::HashMap::new(),
            work_preferences: crate::models::WorkPreferences {
                planning_style: "iterative".to_string(),
                pace: "focused bursts".to_string(),
                autonomy_level: "collaborative".to_string(),
                error_handling: "learn and adapt".to_string(),
                documentation_style: "practical".to_string(),
            },
            behavioral_patterns: crate::models::BehavioralPatterns {
                common_mistakes: vec!["overengineering".to_string()],
                strengths: vec!["vision".to_string(), "resilience".to_string()],
                triggers: vec!["inefficiency".to_string()],
                improvement_areas: vec!["patience".to_string()],
            },
            technical_profile: crate::models::TechnicalProfile {
                preferred_languages: vec!["Python".to_string(), "Rust".to_string()],
                frameworks: vec!["MCP".to_string(), "Docker".to_string()],
                tools: vec!["Claude".to_string(), "Redis".to_string()],
                expertise_areas: vec!["system design".to_string(), "AI integration".to_string()],
                learning_interests: vec!["AGI".to_string(), "distributed systems".to_string()],
            },
            context_awareness: crate::models::ContextAwareness {
                current_project: "LegacyMind".to_string(),
                environment: "Mac Mini + Mac Studio".to_string(),
                instance_role: "system architect".to_string(),
                federation_position: "Human - federation leader".to_string(),
                active_goals: vec!["persistent AI memory".to_string()],
            },
            memory_preferences: crate::models::MemoryPreferences {
                recall_style: "associative".to_string(),
                priority_topics: vec!["project state".to_string(), "instance preferences".to_string()],
                context_depth: "comprehensive".to_string(),
                reference_style: "explicit".to_string(),
            },
            metadata: crate::models::IdentityMetadata {
                version: 1,
                last_updated: chrono::Utc::now(),
                update_count: 0,
                created_at: chrono::Utc::now(),
            },
        }
    }
    
    /// Add to Sam's context
    pub(crate) async fn add_to_sam_context(&self, field: &str, value: serde_json::Value) -> Result<()> {
        let context_key = "Sam:Context";
        self.repository.json_array_append(context_key, &format!(".{}", field), &value).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_context_updated",
            vec![
                ("operation", "add"),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Modify Sam's context
    pub(crate) async fn modify_sam_context(&self, field: &str, value: serde_json::Value) -> Result<()> {
        let context_key = "Sam:Context";
        self.repository.json_set(context_key, &format!(".{}", field), &value).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_context_updated",
            vec![
                ("operation", "modify"),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Delete from Sam's context
    pub(crate) async fn delete_from_sam_context(&self, field: &str, value: Option<serde_json::Value>) -> Result<()> {
        let context_key = "Sam:Context";
        
        if let Some(target_value) = value {
            // Remove specific value from array field
            if let Some(arr) = self.repository.json_get_array(context_key, &format!(".{}", field)).await? {
                let mut new_arr = arr;
                new_arr.retain(|v| v != &target_value);
                self.repository.json_set(context_key, &format!(".{}", field), &json!(new_arr)).await?;
            }
        } else {
            // Delete entire field
            self.repository.json_delete(context_key, &format!(".{}", field)).await?;
        }
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_context_updated",
            vec![
                ("operation", "delete"),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Add to Sam's identity field
    pub(crate) async fn add_to_sam_identity(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        // Process value to ensure correct type
        let processed_value = self.process_identity_value(category, field, value)?;
        
        let identity_key = "Sam:Identity";
        let path = format!(".{}.{}", category, field);
        
        // Handle array fields
        match field {
            // Array fields that support appending
            "common_mistakes" | "strengths" | "triggers" | "improvement_areas" 
            | "preferred_languages" | "frameworks" | "tools" | "expertise_areas" 
            | "learning_interests" | "active_goals" | "core_values" 
            | "boundaries" | "shared_history" | "priority_topics" => {
                self.repository.json_array_append(identity_key, &path, &processed_value).await?;
            }
            
            // Object fields or scalar fields
            _ => {
                self.repository.json_set(identity_key, &path, &processed_value).await?;
            }
        }
        
        // Update metadata
        self.update_sam_identity_metadata(identity_key).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_identity_updated",
            vec![
                ("operation", "add"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Modify Sam's identity field
    pub(crate) async fn modify_sam_identity(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        // Process value to ensure correct type
        let processed_value = self.process_identity_value(category, field, value)?;
        
        let identity_key = "Sam:Identity";
        let path = format!(".{}.{}", category, field);
        
        // Set the field value
        self.repository.json_set(identity_key, &path, &processed_value).await?;
        
        // Update metadata
        self.update_sam_identity_metadata(identity_key).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_identity_updated",
            vec![
                ("operation", "modify"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Delete from Sam's identity field
    pub(crate) async fn delete_from_sam_identity(&self, category: &str, field: &str, value: Option<serde_json::Value>) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        let identity_key = "Sam:Identity";
        let path = format!(".{}.{}", category, field);
        
        if let Some(target_value) = value {
            // Remove specific value from array field
            if let Some(arr) = self.repository.json_get_array(identity_key, &path).await? {
                let mut new_arr = arr;
                new_arr.retain(|v| v != &target_value);
                self.repository.json_set(identity_key, &path, &json!(new_arr)).await?;
            }
        } else {
            // Delete entire field
            self.repository.json_delete(identity_key, &path).await?;
        }
        
        // Update metadata
        self.update_sam_identity_metadata(identity_key).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "sam_identity_updated",
            vec![
                ("operation", "delete"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Update Sam identity metadata
    pub(crate) async fn update_sam_identity_metadata(&self, identity_key: &str) -> Result<()> {
        let now = chrono::Utc::now();
        
        // Increment update count
        self.repository.json_increment(identity_key, ".metadata.update_count", 1).await?;
        
        // Update last_updated timestamp
        self.repository.json_set(
            identity_key, 
            ".metadata.last_updated", 
            &json!(now.to_rfc3339())
        ).await?;
        
        Ok(())
    }
    
    /// Generate comprehensive help response for ui_sam
    pub(crate) fn generate_sam_help_response(&self) -> SamResponse {
        let operations = vec![
            OperationHelp {
                name: "view".to_string(),
                description: "Display Sam's current context and identity data".to_string(),
                required_params: vec![],
                optional_params: vec!["category".to_string(), "field".to_string()],
            },
            OperationHelp {
                name: "add".to_string(),
                description: "Add a value to an array field or set a new field value in Sam's data".to_string(),
                required_params: vec!["category".to_string(), "field".to_string(), "value".to_string()],
                optional_params: vec![],
            },
            OperationHelp {
                name: "modify".to_string(),
                description: "Update an existing field's value in Sam's context or identity".to_string(),
                required_params: vec!["category".to_string(), "field".to_string(), "value".to_string()],
                optional_params: vec![],
            },
            OperationHelp {
                name: "delete".to_string(),
                description: "Remove a specific value from an array field or delete the entire field".to_string(),
                required_params: vec!["category".to_string(), "field".to_string()],
                optional_params: vec!["value".to_string()],
            },
            OperationHelp {
                name: "help".to_string(),
                description: "Show this comprehensive help documentation".to_string(),
                required_params: vec![],
                optional_params: vec![],
            },
        ];

        let categories = vec![
            CategoryHelp {
                name: "context".to_string(),
                description: "General contextual information about current work, environment, and state".to_string(),
                common_fields: vec!["current_work".to_string(), "environment_state".to_string(), "recent_events".to_string()],
            },
            CategoryHelp {
                name: "core_info".to_string(),
                description: "Basic identity information about Sam".to_string(),
                common_fields: vec!["name".to_string(), "primary_purpose".to_string(), "core_values".to_string()],
            },
            CategoryHelp {
                name: "communication".to_string(),
                description: "Sam's communication style and preferences".to_string(),
                common_fields: vec!["tone".to_string(), "verbosity".to_string(), "humor_level".to_string(), "directness".to_string()],
            },
            CategoryHelp {
                name: "relationships".to_string(),
                description: "Information about Sam's relationships with AI instances and others".to_string(),
                common_fields: vec!["trust_level".to_string(), "shared_history".to_string(), "boundaries".to_string()],
            },
            CategoryHelp {
                name: "work_preferences".to_string(),
                description: "Sam's preferences for work style, planning, and collaboration".to_string(),
                common_fields: vec!["planning_style".to_string(), "pace".to_string(), "autonomy_level".to_string()],
            },
            CategoryHelp {
                name: "behavioral_patterns".to_string(),
                description: "Sam's common behaviors, strengths, and areas for improvement".to_string(),
                common_fields: vec!["common_mistakes".to_string(), "strengths".to_string(), "triggers".to_string()],
            },
            CategoryHelp {
                name: "technical_profile".to_string(),
                description: "Sam's technical skills, preferences, and expertise".to_string(),
                common_fields: vec!["preferred_languages".to_string(), "frameworks".to_string(), "expertise_areas".to_string()],
            },
            CategoryHelp {
                name: "context_awareness".to_string(),
                description: "Current context including projects, environment, and goals".to_string(),
                common_fields: vec!["current_project".to_string(), "environment".to_string(), "active_goals".to_string()],
            },
            CategoryHelp {
                name: "memory_preferences".to_string(),
                description: "Sam's preferences for memory management and recall".to_string(),
                common_fields: vec!["recall_style".to_string(), "priority_topics".to_string(), "context_depth".to_string()],
            },
        ];

        let field_types = vec![
            FieldTypeHelp {
                field_type: "text".to_string(),
                description: "String values for names, descriptions, and text content".to_string(),
                examples: vec!["direct".to_string(), "LegacyMind".to_string(), "innovative".to_string()],
            },
            FieldTypeHelp {
                field_type: "numeric".to_string(),
                description: "Floating-point numbers typically ranging from 0.0 to 1.0 for levels/scores".to_string(),
                examples: vec!["0.9".to_string(), "0.95".to_string(), "0.85".to_string()],
            },
            FieldTypeHelp {
                field_type: "array".to_string(),
                description: "Lists of strings for multiple values like skills, goals, or preferences".to_string(),
                examples: vec!["[\"Python\", \"Rust\"]".to_string(), "[\"innovation\", \"persistence\"]".to_string()],
            },
            FieldTypeHelp {
                field_type: "object".to_string(),
                description: "Complex nested structures for detailed configurations".to_string(),
                examples: vec!["{\"current_task\": \"implementing ui_sam\"}".to_string()],
            },
        ];

        let examples = vec![
            ExampleUsage {
                operation: "view".to_string(),
                description: "View Sam's complete context and identity".to_string(),
                example: json!({"operation": "view"}),
            },
            ExampleUsage {
                operation: "modify".to_string(),
                description: "Update Sam's current project in context awareness".to_string(),
                example: json!({
                    "operation": "modify",
                    "category": "context_awareness", 
                    "field": "current_project",
                    "value": "UnifiedIntelligence Phase 5"
                }),
            },
            ExampleUsage {
                operation: "add".to_string(),
                description: "Add context about current work".to_string(),
                example: json!({
                    "operation": "add",
                    "category": "context",
                    "field": "current_work", 
                    "value": "Implementing ui_sam tool"
                }),
            },
            ExampleUsage {
                operation: "add".to_string(),
                description: "Add a new expertise area to Sam's technical profile".to_string(),
                example: json!({
                    "operation": "add",
                    "category": "technical_profile",
                    "field": "expertise_areas",
                    "value": "quantum computing"
                }),
            },
            ExampleUsage {
                operation: "modify".to_string(),
                description: "Update Sam's humor level".to_string(),
                example: json!({
                    "operation": "modify",
                    "category": "communication",
                    "field": "humor_level",
                    "value": 0.95
                }),
            },
        ];

        SamResponse::Help {
            operations,
            categories,
            field_types,
            examples,
        }
    }
}