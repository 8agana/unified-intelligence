use serde_json::json;
use tracing;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::models::{
    UiIdentityParams, IdentityResponse, IdentityOperation, Identity,
    OperationHelp, CategoryHelp, FieldTypeHelp, ExampleUsage, RelationshipDynamics
};
use crate::repository::ThoughtRepository;

/// Trait for identity-related operations
pub trait IdentityHandler {
    /// Handle ui_identity tool
    async fn ui_identity(&self, params: UiIdentityParams) -> Result<IdentityResponse>;
}

impl<R: ThoughtRepository> IdentityHandler for super::ToolHandlers<R> {
    /// Handle ui_identity tool
    async fn ui_identity(&self, params: UiIdentityParams) -> Result<IdentityResponse> {
        let operation = params.operation.unwrap_or(IdentityOperation::View);
        
        tracing::info!(
            "Identity operation '{:?}' for instance '{}' - category: {:?}, field: {:?}",
            operation, self.instance_id, params.category, params.field
        );
        
        match operation {
            IdentityOperation::View => {
                let identity = self.get_or_create_identity().await?;
                Ok(IdentityResponse::View {
                    identity,
                    available_categories: vec![
                        "core_info".to_string(), "communication".to_string(), "relationships".to_string(), 
                        "work_preferences".to_string(), "behavioral_patterns".to_string(), 
                        "technical_profile".to_string(), "context_awareness".to_string(), "memory_preferences".to_string()
                    ],
                })
            }
            
            IdentityOperation::Add => {
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
                
                #[cfg(not(test))]
                self.add_to_identity_document(&category, &field, value).await?;
                #[cfg(test)]
                self.add_to_identity_field(&category, &field, value).await?;
                Ok(IdentityResponse::Updated { 
                    operation: "add".to_string(),
                    category, 
                    field: Some(field),
                    success: true,
                })
            }
            
            IdentityOperation::Modify => {
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
                
                
                #[cfg(not(test))]
                self.modify_identity_document(&category, &field, value).await?;
                #[cfg(test)]
                self.modify_identity_field(&category, &field, value).await?;
                Ok(IdentityResponse::Updated { 
                    operation: "modify".to_string(),
                    category,
                    field: Some(field),
                    success: true,
                })
            }
            
            IdentityOperation::Delete => {
                let category = params.category.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "category".to_string(),
                    reason: "category required for delete operation".to_string(),
                })?;
                let field = params.field.ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "field".to_string(),
                    reason: "field required for delete operation".to_string(),
                })?;
                
                #[cfg(not(test))]
                self.delete_from_identity_document(&category, &field, params.value).await?;
                #[cfg(test)]
                self.delete_from_identity_field(&category, &field, params.value).await?;
                Ok(IdentityResponse::Updated { 
                    operation: "delete".to_string(),
                    category,
                    field: Some(field),
                    success: true,
                })
            }
            
            IdentityOperation::Help => {
                Ok(self.generate_help_response())
            }
        }
    }
}

// Helper methods implementation for ToolHandlers
impl<R: ThoughtRepository> super::ToolHandlers<R> {
    // Helper methods for document-based identity operations
    
    #[cfg(not(test))]
    pub(crate) async fn get_or_create_identity_documents(&self) -> Result<Identity> {
        // First, check if we need to migrate from monolithic format
        let identity_key = format!("{}:identity", self.instance_id);
        if self.repository.get_identity(&identity_key).await?.is_some() {
            // Migrate existing monolithic identity to documents
            tracing::info!("Migrating monolithic identity to document format for {}", self.instance_id);
            self.repository.migrate_identity_to_documents(&self.instance_id).await?;
            
            // Optional: Delete the old monolithic identity after successful migration
            // self.repository.json_delete(&identity_key, ".").await?;
        }
        
        // Get all identity documents
        let documents = self.repository.get_all_identity_documents(&self.instance_id).await?;
        
        if documents.is_empty() {
            // Create default identity documents
            let default_identity = Identity::default_for_instance(&self.instance_id);
            let identity_json = serde_json::to_value(&default_identity)?;
            
            // Convert to documents and save
            let new_documents = crate::identity_documents::conversion::monolithic_to_documents(
                identity_json,
                self.instance_id.clone(),
            )?;
            
            for doc in &new_documents {
                self.repository.save_identity_document(doc).await?;
            }
            
            Ok(default_identity)
        } else {
            // Build identity from documents directly
            let mut identity = Identity::default_for_instance(&self.instance_id);
            
            for doc in documents {
                match doc.field_type.as_str() {
                    "core_info" => identity.core_info = serde_json::from_value(doc.content)?,
                    "communication" => identity.communication = serde_json::from_value(doc.content)?,
                    "work_preferences" => identity.work_preferences = serde_json::from_value(doc.content)?,
                    "behavioral_patterns" => identity.behavioral_patterns = serde_json::from_value(doc.content)?,
                    "technical_profile" => identity.technical_profile = serde_json::from_value(doc.content)?,
                    "context_awareness" => identity.context_awareness = serde_json::from_value(doc.content)?,
                    "memory_preferences" => identity.memory_preferences = serde_json::from_value(doc.content)?,
                    "metadata" => identity.metadata = serde_json::from_value(doc.content)?,
                    field if field.starts_with("relationships:") => {
                        let person = field.strip_prefix("relationships:").unwrap_or(field);
                        let dynamics: RelationshipDynamics = serde_json::from_value(doc.content)?;
                        identity.relationships.insert(person.to_string(), dynamics);
                    }
                    _ => {} // Ignore unknown fields
                }
            }
            
            Ok(identity)
        }
    }
    
    #[cfg(test)]
    pub(crate) async fn get_or_create_identity_documents(&self) -> Result<Identity> {
        // Test version - just use old monolithic storage
        self.get_or_create_identity_monolithic().await
    }
    
    pub(crate) async fn get_or_create_identity(&self) -> Result<Identity> {
        // Backward compatibility wrapper
        #[cfg(not(test))]
        {
            self.get_or_create_identity_documents().await
        }
        #[cfg(test)]
        {
            self.get_or_create_identity_monolithic().await
        }
    }
    
    pub(crate) async fn get_or_create_identity_monolithic(&self) -> Result<Identity> {
        let identity_key = format!("{}:identity", self.instance_id);
        
        // Try to get existing identity using Redis JSON.GET
        if let Some(identity) = self.repository.get_identity(&identity_key).await? {
            Ok(identity)
        } else {
            // Create default identity for this instance
            let identity = Identity::default_for_instance(&self.instance_id);
            self.repository.save_identity(&identity_key, &identity).await?;
            Ok(identity)
        }
    }
    
    #[cfg(not(test))]
    pub(crate) async fn add_to_identity_document(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        // Process value to ensure correct type
        let processed_value = self.process_identity_value(category, field, value)?;
        
        // Determine field type for document
        let field_type = if field == "relationships" || category == "relationships" {
            format!("relationships:{}", field)
        } else {
            category.to_string()
        };
        
        // Get or create document for this field type
        let existing_docs = self.repository.get_identity_documents_by_field(&self.instance_id, &field_type).await?;
        
        let mut document = if let Some(doc) = existing_docs.into_iter().next() {
            doc
        } else {
            // Create new document
            crate::identity_documents::IdentityDocument::new(
                field_type.clone(),
                serde_json::json!({}),
                self.instance_id.clone(),
            )
        };
        
        // Update the document content
        let current_content = document.content.as_object_mut()
            .ok_or_else(|| UnifiedIntelligenceError::Validation {
                field: "content".to_string(),
                reason: "Document content must be an object".to_string(),
            })?;
        
        // Handle array fields
        match field {
            // Array fields that support appending
            "common_mistakes" | "strengths" | "triggers" | "improvement_areas" 
            | "preferred_languages" | "frameworks" | "tools" | "expertise_areas" 
            | "learning_interests" | "active_goals" | "core_values" 
            | "boundaries" | "shared_history" | "priority_topics" => {
                let array = current_content.entry(field)
                    .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                
                if let serde_json::Value::Array(arr) = array {
                    arr.push(processed_value);
                }
            }
            
            // Object fields or scalar fields
            _ => {
                current_content.insert(field.to_string(), processed_value);
            }
        }
        
        // Mark as accessed and update
        document.mark_accessed();
        document.version += 1;
        
        // Save the updated document
        self.repository.save_identity_document(&document).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "identity_updated",
            vec![
                ("operation", "add"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    // Backward compatibility wrapper
    pub(crate) async fn add_to_identity_field(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        self.add_to_identity_document(category, field, value).await
    }
    
    #[cfg(not(test))]
    pub(crate) async fn modify_identity_document(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        // Process value to ensure correct type
        let processed_value = self.process_identity_value(category, field, value)?;
        
        // Determine field type for document
        let field_type = if field == "relationships" || category == "relationships" {
            format!("relationships:{}", field)
        } else {
            category.to_string()
        };
        
        // Get existing document
        let existing_docs = self.repository.get_identity_documents_by_field(&self.instance_id, &field_type).await?;
        
        let mut document = if let Some(doc) = existing_docs.into_iter().next() {
            doc
        } else {
            // Create new document if doesn't exist
            crate::identity_documents::IdentityDocument::new(
                field_type.clone(),
                serde_json::json!({}),
                self.instance_id.clone(),
            )
        };
        
        // Update the document content
        let current_content = document.content.as_object_mut()
            .ok_or_else(|| UnifiedIntelligenceError::Validation {
                field: "content".to_string(),
                reason: "Document content must be an object".to_string(),
            })?;
        
        // Set the field value (replace entire value)
        current_content.insert(field.to_string(), processed_value);
        
        // Mark as accessed and update version
        document.mark_accessed();
        document.version += 1;
        
        // Save the updated document
        self.repository.save_identity_document(&document).await?;
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "identity_updated",
            vec![
                ("operation", "modify"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    // Backward compatibility wrapper
    pub(crate) async fn modify_identity_field(&self, category: &str, field: &str, value: serde_json::Value) -> Result<()> {
        self.modify_identity_document(category, field, value).await
    }
    
    #[cfg(not(test))]
    pub(crate) async fn delete_from_identity_document(&self, category: &str, field: &str, value: Option<serde_json::Value>) -> Result<()> {
        // Validate category
        self.validate_category(category)?;
        
        // Determine field type for document
        let field_type = if field == "relationships" || category == "relationships" {
            format!("relationships:{}", field)
        } else {
            category.to_string()
        };
        
        // Get existing document
        let existing_docs = self.repository.get_identity_documents_by_field(&self.instance_id, &field_type).await?;
        
        if let Some(mut document) = existing_docs.into_iter().next() {
            let current_content = document.content.as_object_mut()
                .ok_or_else(|| UnifiedIntelligenceError::Validation {
                    field: "content".to_string(),
                    reason: "Document content must be an object".to_string(),
                })?;
            
            if let Some(target_value) = value {
                // Remove specific value from array field
                if let Some(field_value) = current_content.get_mut(field) {
                    if let serde_json::Value::Array(arr) = field_value {
                        arr.retain(|v| v != &target_value);
                    }
                }
            } else {
                // Delete entire field
                current_content.remove(field);
                
                // If document is now empty (except metadata), delete the document
                if current_content.is_empty() {
                    self.repository.delete_identity_document(&self.instance_id, &field_type, &document.id).await?;
                    return Ok(());
                }
            }
            
            // Mark as accessed and update version
            document.mark_accessed();
            document.version += 1;
            
            // Save the updated document
            self.repository.save_identity_document(&document).await?;
        }
        
        // Log the change
        self.repository.log_event(
            &self.instance_id,
            "identity_updated",
            vec![
                ("operation", "delete"),
                ("category", category),
                ("field", field),
            ]
        ).await?;
        
        Ok(())
    }
    
    // Backward compatibility wrapper
    pub(crate) async fn delete_from_identity_field(&self, category: &str, field: &str, value: Option<serde_json::Value>) -> Result<()> {
        self.delete_from_identity_document(category, field, value).await
    }
    
    // Metadata is now handled per-document automatically
    pub(crate) async fn update_identity_metadata(&self, _identity_key: &str) -> Result<()> {
        // No-op for backward compatibility
        // Document metadata is updated automatically when documents are saved
        Ok(())
    }
    
    /// Validate category names against the known schema
    pub(crate) fn validate_category(&self, category: &str) -> Result<()> {
        const VALID_CATEGORIES: &[&str] = &[
            "core_info",
            "communication",
            "relationships", 
            "work_preferences",
            "behavioral_patterns",
            "technical_profile",
            "context_awareness",
            "memory_preferences",
        ];
        
        if !VALID_CATEGORIES.contains(&category) {
            return Err(UnifiedIntelligenceError::Validation {
                field: "category".to_string(),
                reason: format!("Invalid category '{}'. Valid categories are: {}", 
                    category, 
                    VALID_CATEGORIES.join(", ")
                )
            });
        }
        
        Ok(())
    }
    
    /// Process identity values to ensure correct types for known numeric and array fields
    pub(crate) fn process_identity_value(&self, category: &str, field: &str, value: serde_json::Value) -> Result<serde_json::Value> {
        // Define numeric fields that should be f32
        let numeric_fields = [
            ("communication", "humor_level"),
            ("communication", "directness"),
            ("work_preferences", "challenge_level"),
            ("work_preferences", "autonomy_level"),
            ("relationships", "trust_level"), // For any relationship.trust_level
        ];
        
        // Define array fields that should be Vec<String>
        let array_fields = [
            ("behavioral_patterns", "common_mistakes"),
            ("behavioral_patterns", "strengths"),
            ("behavioral_patterns", "triggers"),
            ("behavioral_patterns", "improvement_areas"),
            ("technical_profile", "preferred_languages"),
            ("technical_profile", "frameworks"),
            ("technical_profile", "tools"),
            ("technical_profile", "expertise_areas"),
            ("technical_profile", "learning_interests"),
            ("context_awareness", "active_goals"),
            ("memory_preferences", "priority_topics"),
            ("core_info", "core_values"),
        ];
        
        // Check if this is a numeric field
        let is_numeric_field = numeric_fields.iter().any(|(cat, fld)| {
            category == *cat && (field == *fld || field.ends_with(&format!(".{}", fld)))
        });
        
        // Check if this is an array field
        let is_array_field = array_fields.iter().any(|(cat, fld)| {
            category == *cat && field == *fld
        });
        
        if is_numeric_field {
            // Try to convert to number if it's a string
            match &value {
                serde_json::Value::String(s) => {
                    if let Ok(num) = s.parse::<f32>() {
                        return Ok(serde_json::Value::Number(
                            serde_json::Number::from_f64(num as f64)
                                .ok_or_else(|| UnifiedIntelligenceError::Validation {
                                    field: field.to_string(),
                                    reason: "Invalid numeric value".to_string()
                                })?
                        ));
                    }
                }
                serde_json::Value::Number(_) => return Ok(value), // Already a number
                _ => {}
            }
        }
        
        if is_array_field {
            // Try to convert string to array if it's a JSON string
            match &value {
                serde_json::Value::String(s) => {
                    // Check if it looks like a JSON array
                    if s.starts_with('[') && s.ends_with(']') {
                        // Try to parse as JSON array
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                            if parsed.is_array() {
                                return Ok(parsed);
                            }
                        }
                    }
                    // If not a JSON array, split by comma (for comma-separated values)
                    else if s.contains(',') {
                        let items: Vec<String> = s.split(',')
                            .map(|item| item.trim().to_string())
                            .filter(|item| !item.is_empty())
                            .collect();
                        return Ok(serde_json::Value::Array(
                            items.into_iter().map(serde_json::Value::String).collect()
                        ));
                    }
                }
                serde_json::Value::Array(_) => return Ok(value), // Already an array
                _ => {}
            }
        }
        
        Ok(value)
    }
    
    /// Generate comprehensive help response for ui_identity
    pub(crate) fn generate_help_response(&self) -> IdentityResponse {
        let operations = vec![
            OperationHelp {
                name: "view".to_string(),
                description: "Display the current identity structure with all categories and fields".to_string(),
                required_params: vec![],
                optional_params: vec!["category".to_string(), "field".to_string()],
            },
            OperationHelp {
                name: "add".to_string(),
                description: "Add a value to an array field or set a new field value".to_string(),
                required_params: vec!["category".to_string(), "field".to_string(), "value".to_string()],
                optional_params: vec![],
            },
            OperationHelp {
                name: "modify".to_string(),
                description: "Update an existing field's value in the specified category".to_string(),
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
                name: "core_info".to_string(),
                description: "Basic identity information like name, instance type, and core values".to_string(),
                common_fields: vec!["name".to_string(), "instance_id".to_string(), "primary_purpose".to_string(), "core_values".to_string()],
            },
            CategoryHelp {
                name: "communication".to_string(),
                description: "Communication style and preferences including tone, verbosity, and humor".to_string(),
                common_fields: vec!["tone".to_string(), "verbosity".to_string(), "humor_level".to_string(), "directness".to_string(), "formality".to_string()],
            },
            CategoryHelp {
                name: "relationships".to_string(),
                description: "Information about relationships with users, trust levels, and social connections".to_string(),
                common_fields: vec!["trust_level".to_string(), "shared_history".to_string(), "boundaries".to_string()],
            },
            CategoryHelp {
                name: "work_preferences".to_string(),
                description: "Preferences for work style, planning, pace, and collaboration approaches".to_string(),
                common_fields: vec!["planning_style".to_string(), "pace".to_string(), "autonomy_level".to_string(), "challenge_level".to_string()],
            },
            CategoryHelp {
                name: "behavioral_patterns".to_string(),
                description: "Common behaviors, strengths, weaknesses, and triggers".to_string(),
                common_fields: vec!["common_mistakes".to_string(), "strengths".to_string(), "triggers".to_string(), "improvement_areas".to_string()],
            },
            CategoryHelp {
                name: "technical_profile".to_string(),
                description: "Technical skills, preferred languages, frameworks, and expertise areas".to_string(),
                common_fields: vec!["preferred_languages".to_string(), "frameworks".to_string(), "tools".to_string(), "expertise_areas".to_string()],
            },
            CategoryHelp {
                name: "context_awareness".to_string(),
                description: "Current context including project, environment, role, and active goals".to_string(),
                common_fields: vec!["current_project".to_string(), "environment".to_string(), "instance_role".to_string(), "active_goals".to_string()],
            },
            CategoryHelp {
                name: "memory_preferences".to_string(),
                description: "Preferences for memory management, recall style, and priority topics".to_string(),
                common_fields: vec!["recall_style".to_string(), "priority_topics".to_string(), "context_depth".to_string()],
            },
        ];

        let field_types = vec![
            FieldTypeHelp {
                field_type: "text".to_string(),
                description: "String values for names, descriptions, and text content".to_string(),
                examples: vec!["Claude".to_string(), "sarcastic".to_string(), "structured".to_string()],
            },
            FieldTypeHelp {
                field_type: "numeric".to_string(),
                description: "Floating-point numbers typically ranging from 0.0 to 1.0 for levels/scores".to_string(),
                examples: vec!["0.8".to_string(), "0.5".to_string(), "0.9".to_string()],
            },
            FieldTypeHelp {
                field_type: "array".to_string(),
                description: "Lists of strings or objects for multiple values like skills, goals, or mistakes".to_string(),
                examples: vec!["[\"Rust\", \"TypeScript\"]".to_string(), "[\"planning\", \"execution\"]".to_string()],
            },
            FieldTypeHelp {
                field_type: "object".to_string(),
                description: "Complex nested structures for relationships or detailed configurations".to_string(),
                examples: vec!["{\"Sam\": {\"trust_level\": 0.9}}".to_string()],
            },
        ];

        let examples = vec![
            ExampleUsage {
                operation: "view".to_string(),
                description: "View complete identity structure".to_string(),
                example: json!({"operation": "view"}),
            },
            ExampleUsage {
                operation: "modify".to_string(),
                description: "Update humor level in communication preferences".to_string(),
                example: json!({
                    "operation": "modify",
                    "category": "communication", 
                    "field": "humor_level",
                    "value": 0.7
                }),
            },
            ExampleUsage {
                operation: "add".to_string(),
                description: "Add a new programming language to technical profile".to_string(),
                example: json!({
                    "operation": "add",
                    "category": "technical_profile",
                    "field": "preferred_languages", 
                    "value": "Python"
                }),
            },
            ExampleUsage {
                operation: "delete".to_string(),
                description: "Remove a specific goal from active goals".to_string(),
                example: json!({
                    "operation": "delete",
                    "category": "context_awareness",
                    "field": "active_goals",
                    "value": "old goal"
                }),
            },
            ExampleUsage {
                operation: "modify".to_string(),
                description: "Set trust level for a specific relationship".to_string(),
                example: json!({
                    "operation": "modify",
                    "category": "relationships",
                    "field": "Sam.trust_level",
                    "value": 0.9
                }),
            },
        ];

        IdentityResponse::Help {
            operations,
            categories,
            field_types,
            examples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MockThoughtRepository;
    use crate::validation::InputValidator;
    use crate::search_optimization::SearchCache;
    
    fn create_test_handler() -> super::super::ToolHandlers<MockThoughtRepository> {
        let repository = Arc::new(MockThoughtRepository::new());
        let validator = Arc::new(InputValidator::new());
        let search_cache = Arc::new(std::sync::Mutex::new(SearchCache::new(300))); // 5 minute TTL
        let search_available = Arc::new(std::sync::atomic::AtomicBool::new(true));
        
        super::super::ToolHandlers::new(
            repository,
            "test".to_string(),
            validator,
            search_cache,
            search_available,
        )
    }
    
    #[test]
    fn test_process_identity_value_numeric_fields() {
        let handler = create_test_handler();
        
        // Test numeric field conversion from string to f32
        let test_cases = vec![
            ("communication", "humor_level", "0.75"),
            ("communication", "directness", "0.9"),
            ("work_preferences", "challenge_level", "0.8"),
            ("work_preferences", "autonomy_level", "0.85"),
            ("relationships", "trust_level", "0.95"),
        ];
        
        for (category, field, value_str) in test_cases {
            let input = json!(value_str);
            let result = handler.process_identity_value(category, field, input).unwrap();
            
            // Verify it's a number
            assert!(result.is_f64() || result.is_u64() || result.is_i64(), 
                    "Result should be numeric for {}.{}", category, field);
            
            // Compare with epsilon for floating point
            let result_f64 = result.as_f64().unwrap();
            let expected_f64 = value_str.parse::<f64>().unwrap();
            assert!((result_f64 - expected_f64).abs() < 0.0001, 
                    "Value mismatch for {}.{}: {} vs {}", 
                    category, field, result_f64, expected_f64);
        }
    }
    
    #[test]
    fn test_process_identity_value_numeric_already_correct() {
        let handler = create_test_handler();
        
        // Test that already-correct numeric values are preserved
        let test_cases = vec![
            ("communication", "humor_level", json!(0.75)),
            ("communication", "directness", json!(0.9)),
            ("work_preferences", "challenge_level", json!(0.8)),
        ];
        
        for (category, field, value) in test_cases {
            let result = handler.process_identity_value(category, field, value.clone()).unwrap();
            assert_eq!(result, value, "Value should be unchanged for {}.{}", category, field);
        }
    }
    
    #[test]
    fn test_process_identity_value_array_fields() {
        let handler = create_test_handler();
        
        // Test array field conversion from JSON string to array
        let test_cases = vec![
            (
                "behavioral_patterns",
                "strengths",
                json!("[\"fast execution\", \"creative solutions\"]"),
                json!(["fast execution", "creative solutions"])
            ),
            (
                "technical_profile",
                "preferred_languages",
                json!("[\"Rust\", \"TypeScript\"]"),
                json!(["Rust", "TypeScript"])
            ),
            (
                "technical_profile",
                "expertise_areas",
                json!("[\"MCP development\", \"Redis\"]"),
                json!(["MCP development", "Redis"])
            ),
        ];
        
        for (category, field, input, expected) in test_cases {
            let result = handler.process_identity_value(category, field, input).unwrap();
            assert_eq!(result, expected, "Failed for {}.{}", category, field);
        }
    }
    
    #[test]
    fn test_process_identity_value_array_comma_separated() {
        let handler = create_test_handler();
        
        // Test comma-separated string conversion to array
        let result = handler.process_identity_value(
            "behavioral_patterns",
            "strengths",
            json!("fast execution, creative solutions, systematic debugging")
        ).unwrap();
        
        assert_eq!(
            result,
            json!(["fast execution", "creative solutions", "systematic debugging"])
        );
    }
    
    #[test]
    fn test_process_identity_value_array_already_correct() {
        let handler = create_test_handler();
        
        // Test that already-correct arrays are preserved
        let value = json!(["Rust", "TypeScript", "Python"]);
        let result = handler.process_identity_value(
            "technical_profile",
            "preferred_languages",
            value.clone()
        ).unwrap();
        
        assert_eq!(result, value);
    }
    
    #[test]
    fn test_process_identity_value_non_special_fields() {
        let handler = create_test_handler();
        
        // Test that non-special fields are passed through unchanged
        let test_cases = vec![
            ("core_info", "name", json!("Claude")),
            ("communication", "tone", json!("friendly")),
            ("work_preferences", "planning_style", json!("structured")),
            ("some_category", "some_field", json!("some value")),
        ];
        
        for (category, field, value) in test_cases {
            let result = handler.process_identity_value(category, field, value.clone()).unwrap();
            assert_eq!(result, value, "Value should be unchanged for {}.{}", category, field);
        }
    }
    
    #[test]
    fn test_process_identity_value_invalid_numeric_string() {
        let handler = create_test_handler();
        
        // Test that invalid numeric strings are passed through unchanged
        let result = handler.process_identity_value(
            "communication",
            "humor_level",
            json!("not a number")
        ).unwrap();
        
        assert_eq!(result, json!("not a number"));
    }
    
    #[test]
    fn test_process_identity_value_invalid_json_array_string() {
        let handler = create_test_handler();
        
        // Test that invalid JSON array strings are passed through unchanged
        let result = handler.process_identity_value(
            "behavioral_patterns",
            "strengths",
            json!("[invalid json")
        ).unwrap();
        
        assert_eq!(result, json!("[invalid json"));
    }
}