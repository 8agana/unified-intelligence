pub mod help;
pub mod knowledge;
pub mod recall;
/// Handler modules for UnifiedIntelligence MCP tools
pub mod thoughts;

use crate::config::Config;
use crate::qdrant_service::{QdrantService, QdrantServiceTrait};
use crate::redis::RedisManager;
use crate::repository_traits::{ThoughtRepository, KnowledgeRepository};
use crate::validation::InputValidator;
use crate::visual::VisualOutput;
use std::sync::Arc;

// Re-export handler traits from submodules
pub use help::HelpHandler;
pub use recall::RecallHandler;

/// Handler for MCP tool operations
pub struct ToolHandlers<R: ThoughtRepository + KnowledgeRepository> {
    pub(crate) repository: Arc<R>,
    pub(crate) instance_id: String,
    pub(crate) validator: Arc<InputValidator>,
    pub(crate) visual: VisualOutput,
    pub(crate) recall: RecallHandler<R>,
    pub(crate) help: HelpHandler,
    pub(crate) redis_manager: Arc<RedisManager>,
    pub(crate) qdrant_service: Arc<dyn QdrantServiceTrait>,
    pub(crate) config: Arc<Config>,
}

impl<R: ThoughtRepository + KnowledgeRepository> ToolHandlers<R> {
    pub fn new(
        repository: Arc<R>,
        instance_id: String,
        validator: Arc<InputValidator>,
        redis_manager: Arc<RedisManager>,
        qdrant_service: Arc<dyn QdrantServiceTrait>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            repository: repository.clone(),
            instance_id: instance_id.clone(),
            validator,
            visual: VisualOutput::new(),
            recall: RecallHandler::new(repository.clone(), instance_id.clone()),
            help: HelpHandler::new(instance_id.clone()),
            redis_manager,
            qdrant_service,
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository_traits::{ThoughtRepository, KnowledgeRepository};
    use crate::qdrant_service::{QdrantServiceTrait, MockQdrantServiceTrait};
    use crate::redis::RedisManager;
    use crate::config::Config;

    // Define a mock that implements both ThoughtRepository and KnowledgeRepository
    mockall::mock! {
        CombinedMockRepository {
            // This is needed because the trait has a generic lifetime parameter
            // for the `search_entities` method.
            // See: https://docs.rs/mockall/latest/mockall/#mocking-traits-with-lifetimes
            for<'a> impl KnowledgeRepository for CombinedMockRepository {
                async fn search_entities(&self, query: &str, scope: &crate::models::KnowledgeScope, entity_type: Option<&'a crate::models::EntityType>, limit: usize) -> crate::error::Result<Vec<crate::models::KnowledgeNode>> {
                    unimplemented!()
                }
            }
        }

        impl ThoughtRepository for CombinedMockRepository {
            async fn save_thought(&self, thought: &crate::models::ThoughtRecord) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn save_chain_metadata(&self, metadata: &crate::models::ChainMetadata) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn chain_exists(&self, chain_id: &str) -> crate::error::Result<bool> {
                unimplemented!()
            }
            async fn get_thought(&self, instance: &str, thought_id: &str) -> crate::error::Result<Option<crate::models::ThoughtRecord>> {
                unimplemented!()
            }
            async fn get_chain_thoughts(&self, instance: &str, chain_id: &str) -> crate::error::Result<Vec<crate::models::ThoughtRecord>> {
                unimplemented!()
            }
            async fn search_thoughts(&self, instance: &str, query: &str, offset: i64, limit: i64) -> crate::error::Result<Vec<crate::models::ThoughtRecord>> {
                unimplemented!()
            }
        }

        // The rest of the KnowledgeRepository methods are implemented here
        // outside the for<'a> block, as they don't have lifetime parameters
        impl KnowledgeRepository for CombinedMockRepository {
            async fn create_entity(&self, node: crate::models::KnowledgeNode) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn get_entity(&self, id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<crate::models::KnowledgeNode> {
                unimplemented!()
            }
            async fn get_entity_by_name(&self, name: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<crate::models::KnowledgeNode> {
                unimplemented!()
            }
            async fn update_entity(&self, node: crate::models::KnowledgeNode) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn delete_entity(&self, id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn create_relation(&self, relation: crate::models::KnowledgeRelation) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn get_relations(&self, entity_id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<Vec<crate::models::KnowledgeRelation>> {
                unimplemented!()
            }
            async fn update_name_index(&self, name: &str, id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn set_active_entity(&self, session_key: &str, entity_id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<()> {
                unimplemented!()
            }
            async fn add_thought_to_entity(&self, entity_name: &str, thought_id: &str, scope: &crate::models::KnowledgeScope) -> crate::error::Result<()> {
                unimplemented!()
            }
        }
    }

    async fn create_test_handler() -> ToolHandlers<MockCombinedMockRepository> {
        let repository = Arc::new(MockCombinedMockRepository::new());
        let validator = Arc::new(InputValidator::new());

        let config = Arc::new(Config::default());
        let redis_manager = Arc::new(RedisManager::new_with_config(&config).await.unwrap());
        let qdrant_service = Arc::new(MockQdrantServiceTrait::new());

        ToolHandlers::new(
            repository,
            "test".to_string(),
            validator,
            redis_manager,
            qdrant_service,
            config,
        )
    }
}
