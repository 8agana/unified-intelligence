// Re-export commonly used types for tests
pub mod models;
pub mod error;
pub mod config;
pub mod redis;
pub mod repository;
pub mod repository_traits;
pub mod search_optimization;
pub mod validation;
pub mod identity_documents;
pub mod lua_scripts;
pub mod frameworks;
pub mod rate_limit;
pub mod visual;
pub mod retry;
pub mod circuit_breaker;
pub mod redis_abstraction;
pub mod search_enhancements;

// Re-export specific types that tests might need
pub use models::ThoughtRecord;
pub use error::{Result, UnifiedIntelligenceError};