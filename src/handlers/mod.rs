/// Handler modules for UnifiedIntelligence MCP tools
pub mod thoughts;
pub mod recall;

use std::sync::Arc;
use crate::repository::ThoughtRepository;
use crate::validation::InputValidator;
use crate::visual::VisualOutput;

// Re-export handler traits from submodules
pub use recall::RecallHandler;

/// Handler for MCP tool operations
pub struct ToolHandlers<R: ThoughtRepository> {
    pub(crate) repository: Arc<R>,
    pub(crate) instance_id: String,
    pub(crate) validator: Arc<InputValidator>,
    pub(crate) visual: VisualOutput,
    pub(crate) recall: RecallHandler<R>,
}

impl<R: ThoughtRepository> ToolHandlers<R> {
    pub fn new(
        repository: Arc<R>,
        instance_id: String,
        validator: Arc<InputValidator>,
    ) -> Self {
        Self {
            repository: repository.clone(),
            instance_id: instance_id.clone(),
            validator,
            visual: VisualOutput::new(),
            recall: RecallHandler::new(repository.clone(), instance_id.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MockThoughtRepository;
    
    fn create_test_handler() -> ToolHandlers<MockThoughtRepository> {
        let repository = Arc::new(MockThoughtRepository::new());
        let validator = Arc::new(InputValidator::new());
        
        ToolHandlers::new(
            repository,
            "test".to_string(),
            validator,
        )
    }
}