use crate::error::Result;
use crate::models::{
    ChainMetadata, EntityType, KnowledgeNode, KnowledgeRelation, KnowledgeScope, ThoughtRecord,
};
use async_trait::async_trait;

#[cfg(test)]
use mockall::automock;

#[async_trait]
#[cfg_attr(test, automock)]
pub trait ThoughtRepository: Send + Sync + 'static {
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()>;
    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()>;
    async fn chain_exists(&self, chain_id: &str) -> Result<bool>;
    async fn get_thought(&self, instance: &str, thought_id: &str) -> Result<Option<ThoughtRecord>>;
    async fn get_chain_thoughts(
        &self,
        instance: &str,
        chain_id: &str,
    ) -> Result<Vec<ThoughtRecord>>;
    async fn search_thoughts(
        &self,
        instance: &str,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ThoughtRecord>>;
}

#[async_trait]
#[cfg_attr(test, automock)]
pub trait KnowledgeRepository: Send + Sync + 'static {
    async fn create_entity(&self, node: KnowledgeNode) -> Result<()>;
    async fn get_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<KnowledgeNode>;
    async fn get_entity_by_name(&self, name: &str, scope: &KnowledgeScope)
    -> Result<KnowledgeNode>;
    async fn update_entity(&self, node: KnowledgeNode) -> Result<()>;
    async fn delete_entity(&self, id: &str, scope: &KnowledgeScope) -> Result<()>;
    async fn search_entities(
        &self,
        query: &str,
        scope: &KnowledgeScope,
        entity_type: Option<&EntityType>,
        limit: usize,
    ) -> Result<Vec<KnowledgeNode>>;
    async fn create_relation(&self, relation: KnowledgeRelation) -> Result<()>;
    async fn get_relations(
        &self,
        entity_id: &str,
        scope: &KnowledgeScope,
    ) -> Result<Vec<KnowledgeRelation>>;
    async fn update_name_index(&self, name: &str, id: &str, scope: &KnowledgeScope) -> Result<()>;
    async fn set_active_entity(
        &self,
        session_key: &str,
        entity_id: &str,
        scope: &KnowledgeScope,
    ) -> Result<()>;
    async fn add_thought_to_entity(
        &self,
        entity_name: &str,
        thought_id: &str,
        scope: &KnowledgeScope,
    ) -> Result<()>;
}
