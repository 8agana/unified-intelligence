/// Handler modules for UnifiedIntelligence MCP tools
pub mod identity;
pub mod sam;
pub mod search;
pub mod thoughts;

use std::sync::Arc;
use crate::repository::ThoughtRepository;
#[cfg(not(test))]
use crate::search_optimization::SearchCache;
use crate::validation::InputValidator;
use crate::visual::VisualOutput;

// Re-export handler traits from submodules
pub use identity::IdentityHandler;
pub use sam::SamHandler;
pub use search::SearchHandler;
pub use thoughts::ThoughtsHandler;

/// Handler for MCP tool operations
pub struct ToolHandlers<R: ThoughtRepository> {
    pub(crate) repository: Arc<R>,
    pub(crate) instance_id: String,
    pub(crate) validator: Arc<InputValidator>,
    pub(crate) _search_cache: Arc<std::sync::Mutex<SearchCache>>,
    pub(crate) search_available: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) visual: VisualOutput,
}

impl<R: ThoughtRepository> ToolHandlers<R> {
    pub fn new(
        repository: Arc<R>,
        instance_id: String,
        validator: Arc<InputValidator>,
        search_cache: Arc<std::sync::Mutex<SearchCache>>,
        search_available: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            repository,
            instance_id,
            validator,
            _search_cache: search_cache,
            search_available,
            visual: VisualOutput::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MockThoughtRepository;
    use crate::search_optimization::SearchCache;
    
    fn create_test_handler() -> ToolHandlers<MockThoughtRepository> {
        let repository = Arc::new(MockThoughtRepository::new());
        let validator = Arc::new(InputValidator::new());
        let search_cache = Arc::new(std::sync::Mutex::new(SearchCache::new(300))); // 5 minute TTL
        let search_available = Arc::new(std::sync::atomic::AtomicBool::new(true));
        
        ToolHandlers::new(
            repository,
            "test".to_string(),
            validator,
            search_cache,
            search_available,
        )
    }
}