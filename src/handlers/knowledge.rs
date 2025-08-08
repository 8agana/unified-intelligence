use tracing;
use chrono::Utc;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{
    KnowledgeNode, KnowledgeRelation, KnowledgeResponse, 
    CreateEntityParams, EntityType, KnowledgeScope,
    SearchParams, SetActiveParams, GetEntityParams,
    CreateRelationParams, GetRelationsParams, UpdateEntityParams,
    DeleteEntityParams, NodeMetadata, RelationMetadata, UiKnowledgeParams
};
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};

/// Trait for knowledge graph operations
pub trait KnowledgeHandler {
    /// Handle ui_knowledge tool
    async fn ui_knowledge(&self, params: UiKnowledgeOperation) -> crate::error::Result<KnowledgeResponse>;
}

impl<R: ThoughtRepository + KnowledgeRepository> KnowledgeHandler for super::ToolHandlers<R> {
    async fn ui_knowledge(&self, params: UiKnowledgeOperation) -> Result<KnowledgeResponse> {
        match params {
            UiKnowledgeOperation::Create(params) => self.create_entity(params).await,
            UiKnowledgeOperation::Search(params) => self.search_entities(params).await,
            UiKnowledgeOperation::SetActive(params) => self.set_active_entity(params).await,
            UiKnowledgeOperation::GetEntity(params) => self.get_entity(params).await,
            UiKnowledgeOperation::CreateRelation(params) => self.create_relation(params).await,
            UiKnowledgeOperation::GetRelations(params) => self.get_relations(params).await,
            UiKnowledgeOperation::UpdateEntity(params) => self.update_entity(params).await,
            UiKnowledgeOperation::DeleteEntity(params) => self.delete_entity(params).await,
        }
    }
}

impl<R: ThoughtRepository + KnowledgeRepository> super::ToolHandlers<R> {
    async fn create_entity(&self, params: CreateEntityParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Creating entity '{}' in {} scope", 
            params.name, 
            params.scope
        );
        
        // Check if entity already exists
        if let Ok(existing) = self.repository.get_entity_by_name(
            &params.name, 
            &params.scope
        ).await {
            return Ok(KnowledgeResponse {
                status: "exists".to_string(),
                entity_id: Some(existing.id.clone()),
                entities: Some(vec![existing]),
                relations: None,
                message: Some(format!("Entity '{}' already exists", params.name)),
            });
        }
        
        // Create new entity
        let node = KnowledgeNode {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            display_name: params.display_name.unwrap_or_else(|| params.name.clone()),
            entity_type: params.entity_type,
            scope: params.scope.clone(),
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
            &params.name,
            &node.id,
            &params.scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "created".to_string(),
            entity_id: Some(node.id.clone()),
            entities: Some(vec![node]),
            relations: None,
            message: Some(format!("Entity '{}' created successfully", params.name)),
        })
    }
    
    async fn search_entities(&self, params: SearchParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Searching for '{}' in {} scope",
            params.query,
            params.scope
        );
        
        let entities = self.repository.search_entities(
            &params.query,
            &params.scope,
            params.entity_type.as_ref(),
            params.limit
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: None,
            entities: Some(entities.clone()),
            relations: None,
            message: Some(format!("Found {} entities", entities.len())),
        })
    }
    
    async fn set_active_entity(&self, params: SetActiveParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Setting active entity '{}' in {} scope",
            params.entity_id,
            params.scope
        );
        
        // Verify entity exists
        let entity = self.repository.get_entity(
            &params.entity_id,
            &params.scope
        ).await?;
        
        // Store active entity in Redis session key
        let session_key = format!("{}:KG:active_entity", self.instance_id);
        self.repository.set_active_entity(
            &session_key,
            &params.entity_id,
            &params.scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "active".to_string(),
            entity_id: Some(entity.id.clone()),
            entities: Some(vec![entity]),
            relations: None,
            message: Some(format!("Entity set as active context")),
        })
    }
    
    async fn get_entity(&self, params: GetEntityParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Getting entity '{}' from {} scope",
            params.entity_id,
            params.scope
        );
        
        let entity = self.repository.get_entity(
            &params.entity_id,
            &params.scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: Some(entity.id.clone()),
            entities: Some(vec![entity]),
            relations: None,
            message: Some("Entity retrieved successfully".to_string()),
        })
    }
    
    async fn create_relation(&self, params: CreateRelationParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Creating relation '{}' from {} to {} in {} scope",
            params.relationship_type,
            params.from_entity_id,
            params.to_entity_id,
            params.scope
        );
        
        // Verify both entities exist
        let _from_entity = self.repository.get_entity(
            &params.from_entity_id,
            &params.scope
        ).await?;
        
        let _to_entity = self.repository.get_entity(
            &params.to_entity_id,
            &params.scope
        ).await?;
        
        // Create relation
        let relation = KnowledgeRelation {
            id: Uuid::new_v4().to_string(),
            from_entity_id: params.from_entity_id,
            to_entity_id: params.to_entity_id,
            relationship_type: params.relationship_type,
            scope: params.scope,
            created_at: Utc::now(),
            created_by: self.instance_id.clone(),
            attributes: params.attributes.unwrap_or_default(),
            metadata: RelationMetadata {
                bidirectional: params.bidirectional,
                weight: params.weight,
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
    
    async fn get_relations(&self, params: GetRelationsParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Getting relations for entity '{}' in {} scope",
            params.entity_id,
            params.scope
        );
        
        let relations = self.repository.get_relations(
            &params.entity_id,
            &params.scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "success".to_string(),
            entity_id: Some(params.entity_id),
            entities: None,
            relations: Some(relations.clone()),
            message: Some(format!("Found {} relations", relations.len())),
        })
    }
    
    async fn update_entity(&self, params: UpdateEntityParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Updating entity '{}' in {} scope",
            params.entity_id,
            params.scope
        );
        
        // Get existing entity
        let mut entity = self.repository.get_entity(
            &params.entity_id,
            &params.scope
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
    
    async fn delete_entity(&self, params: DeleteEntityParams) -> Result<KnowledgeResponse> {
        tracing::info!(
            "Deleting entity '{}' from {} scope",
            params.entity_id,
            params.scope
        );
        
        // Verify entity exists before deletion
        let entity = self.repository.get_entity(
            &params.entity_id,
            &params.scope
        ).await?;
        
        // Delete the entity
        self.repository.delete_entity(
            &params.entity_id,
            &params.scope
        ).await?;
        
        Ok(KnowledgeResponse {
            status: "deleted".to_string(),
            entity_id: Some(params.entity_id),
            entities: None,
            relations: None,
            message: Some(format!("Entity '{}' deleted successfully", entity.name)),
        })
    }
}