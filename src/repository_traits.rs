use crate::error::Result;
use crate::models::{ChainMetadata, ThoughtRecord};
use async_trait::async_trait;

#[async_trait]
pub trait ThoughtRepository: Send + Sync + 'static {
    async fn save_thought(&self, thought: &ThoughtRecord) -> Result<()>;
    async fn save_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()>;
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
