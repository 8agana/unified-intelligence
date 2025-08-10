pub mod help;
pub mod knowledge;
pub mod recall;
/// Handler modules for UnifiedIntelligence MCP tools
pub mod thoughts;

#[cfg(test)]
mod test_handlers;

use crate::config::Config;
use crate::qdrant_service::QdrantServiceTrait;
use crate::redis::RedisManager;
use crate::repository_traits::{KnowledgeRepository, ThoughtRepository};
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
