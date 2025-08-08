use tracing;
use chrono::Utc;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{
    KnowledgeNode, KnowledgeRelation, KnowledgeResponse, 
    UiKnowledgeParams, NodeMetadata, RelationMetadata
};
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};

/// Trait for knowledge graph operations
pub trait KnowledgeHandler {
    /// Handle ui_knowledge tool
    async fn ui_knowledge(&self, params: UiKnowledgeParams) -> crate::error::Result<KnowledgeResponse>;
}

impl<R: ThoughtRepository + KnowledgeRepository> KnowledgeHandler for super::ToolHandlers<R> {
    async fn ui_knowledge(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        match params.mode.as_str() {
            "create" => self.create_entity(params).await,
            "search" => self.search_entities(params).await,
            "set_active" => self.set_active_entity(params).await,
            "get_entity" => self.get_entity(params).await,
            "create_relation" => self.create_relation(params).await,
            "get_relations" => self.get_relations(params).await,
            "update_entity" => self.update_entity(params).await,
            "delete_entity" => self.delete_entity(params).await,
            _ => Err(crate::error::UnifiedIntelligenceError::Validation {
                field: "mode".to_string(),
                reason: format!("Invalid mode: {}. Valid modes are: create, search, set_active, get_entity, create_relation, get_relations, update_entity, delete_entity", params.mode)
            })
        }
    }
}

impl<R: ThoughtRepository + KnowledgeRepository> super::ToolHandlers<R> {
    async fn create_entity(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for create mode
        let name = params.name.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "name".to_string(),
                reason: "name is required for create mode".to_string()
            }
        })?;
        let entity_type = params.entity_type.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_type".to_string(),
                reason: "entity_type is required for create mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Creating entity '{}' in {} scope", 
            name, 
            scope
        );
        
        // Check if entity already exists
        if let Ok(existing) = self.repository.get_entity_by_name(
            &name, 
            &scope
        ).await {
            return Ok(KnowledgeResponse {
                status: "exists".to_string(),
                entity_id: Some(existing.id.clone()),
                entities: Some(vec![existing]),
                relations: None,
                message: Some(format!("Entity '{}' already exists", name)),
            });
        }
        
        // Create new entity
        let node = KnowledgeNode {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            display_name: params.display_name.unwrap_or_else(|| name.clone()),
            entity_type,
            scope: scope.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: self.instance_id.clone(),
            attributes: params.attributes.unwrap_or_default(),
            tags: params.tags.unwrap_or_default(),
            thought_ids: vec![],
            embedding: None,
            metadata: NodeMetadata {
                auto_extracted: false,
                extraction_source: None,
                extraction_timestamp: None,
            },
        };
        
        // Store in Redis
        self.repository.create_entity(node.clone()).await?;
        
        // Update index
        self.repository.update_name_index(
            &name,
            &node.id,
            &scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "created".to_string(),
            entity_id: Some(node.id.clone()),
            entities: Some(vec![node]),
            relations: None,
            message: Some(format!("Entity '{}' created successfully", name)),
        })
    }
    
    async fn search_entities(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for search mode
        let query = params.query.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "query".to_string(),
                reason: "query is required for search mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        let limit = params.limit.unwrap_or(10);
        
        tracing::info!(
            "Searching for '{}' in {} scope",
            query,
            scope
        );
        
        let entities = self.repository.search_entities(
            &query,
            &scope,
            params.entity_type.as_ref(),
            limit
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: None,
            entities: Some(entities.clone()),
            relations: None,
            message: Some(format!("Found {} entities", entities.len())),
        })
    }
    
    async fn set_active_entity(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for set_active mode
        let entity_id = params.entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_id".to_string(),
                reason: "entity_id is required for set_active mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Setting active entity '{}' in {} scope",
            entity_id,
            scope
        );
        
        // Verify entity exists
        let entity = self.repository.get_entity(
            &entity_id,
            &scope
        ).await?;
        
        // Store active entity in Redis session key
        let session_key = format!("{}:KG:active_entity", self.instance_id);
        self.repository.set_active_entity(
            &session_key,
            &entity_id,
            &scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "active".to_string(),
            entity_id: Some(entity.id.clone()),
            entities: Some(vec![entity]),
            relations: None,
            message: Some(format!("Entity set as active context")),
        })
    }
    
    async fn get_entity(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for get_entity mode
        let entity_id = params.entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_id".to_string(),
                reason: "entity_id is required for get_entity mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Getting entity '{}' from {} scope",
            entity_id,
            scope
        );
        
        let entity = self.repository.get_entity(
            &entity_id,
            &scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: Some(entity.id.clone()),
            entities: Some(vec![entity]),
            relations: None,
            message: Some("Entity retrieved successfully".to_string()),
        })
    }
    
    async fn create_relation(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for create_relation mode
        let from_entity_id = params.from_entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "from_entity_id".to_string(),
                reason: "from_entity_id is required for create_relation mode".to_string()
            }
        })?;
        let to_entity_id = params.to_entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "to_entity_id".to_string(),
                reason: "to_entity_id is required for create_relation mode".to_string()
            }
        })?;
        let relationship_type = params.relationship_type.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "relationship_type".to_string(),
                reason: "relationship_type is required for create_relation mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        let bidirectional = params.bidirectional.unwrap_or(false);
        let weight = params.weight.unwrap_or(1.0);
        
        tracing::info!(
            "Creating relation '{}' from {} to {} in {} scope",
            relationship_type,
            from_entity_id,
            to_entity_id,
            scope
        );
        
        // Verify both entities exist
        let _from_entity = self.repository.get_entity(
            &from_entity_id,
            &scope
        ).await?;
        
        let _to_entity = self.repository.get_entity(
            &to_entity_id,
            &scope
        ).await?;
        
        // Create relation
        let relation = KnowledgeRelation {
            id: Uuid::new_v4().to_string(),
            from_entity_id,
            to_entity_id,
            relationship_type,
            scope,
            created_at: Utc::now(),
            created_by: self.instance_id.clone(),
            attributes: params.attributes.unwrap_or_default(),
            metadata: RelationMetadata {
                bidirectional,
                weight,
            },
        };
        
        self.repository.create_relation(relation.clone()).await?;
        
        Ok(KnowledgeResponse {
            status: "created".to_string(),
            entity_id: None,
            entities: None,
            relations: Some(vec![relation]),
            message: Some("Relation created successfully".to_string()),
        })
    }
    
    async fn get_relations(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for get_relations mode
        let entity_id = params.entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_id".to_string(),
                reason: "entity_id is required for get_relations mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Getting relations for entity '{}' in {} scope",
            entity_id,
            scope
        );
        
        let relations = self.repository.get_relations(
            &entity_id,
            &scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: Some(entity_id),
            entities: None,
            relations: Some(relations.clone()),
            message: Some(format!("Found {} relations", relations.len())),
        })
    }
    
    async fn update_entity(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for update_entity mode
        let entity_id = params.entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_id".to_string(),
                reason: "entity_id is required for update_entity mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Updating entity '{}' in {} scope",
            entity_id,
            scope
        );
        
        // Get existing entity
        let mut entity = self.repository.get_entity(
            &entity_id,
            &scope
        ).await?;
        
        // Update fields
        if let Some(display_name) = params.display_name {
            entity.display_name = display_name;
        }
        
        if let Some(attributes) = params.attributes {
            entity.attributes = attributes;
        }
        
        if let Some(tags) = params.tags {
            entity.tags = tags;
        }
        
        entity.updated_at = Utc::now();
        
        // Save updated entity
        self.repository.update_entity(entity.clone()).await?;
        
        Ok(KnowledgeResponse {
            status: "updated".to_string(),
            entity_id: Some(entity.id.clone()),
            entities: Some(vec![entity]),
            relations: None,
            message: Some("Entity updated successfully".to_string()),
        })
    }
    
    async fn delete_entity(&self, params: UiKnowledgeParams) -> Result<KnowledgeResponse> {
        // Validate required fields for delete_entity mode
        let entity_id = params.entity_id.ok_or_else(|| {
            crate::error::UnifiedIntelligenceError::Validation {
                field: "entity_id".to_string(),
                reason: "entity_id is required for delete_entity mode".to_string()
            }
        })?;
        let scope = params.scope.unwrap_or_default();
        
        tracing::info!(
            "Deleting entity '{}' from {} scope",
            entity_id,
            scope
        );
        
        // Verify entity exists before deletion
        let entity = self.repository.get_entity(
            &entity_id,
            &scope
        ).await?;
        
        // Delete the entity
        self.repository.delete_entity(
            &entity_id,
            &scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "deleted".to_string(),
            entity_id: Some(entity_id),
            entities: None,
            relations: None,
            message: Some(format!("Entity '{}' deleted successfully", entity.name)),
        })
    }
}