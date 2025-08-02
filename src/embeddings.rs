use anyhow::Result;
use async_openai::{
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
    Client,
    config::OpenAIConfig,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::UnifiedIntelligenceError;
use crate::redis::RedisManager;

#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    input: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbedding>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbedding {
    embedding: Vec<f32>,
}

pub async fn generate_openai_embedding(
    text: &str,
    openai_api_key: &str,
    redis_manager: &RedisManager, // Pass RedisManager for caching
) -> Result<Vec<f32>> {
    // Check cache first
    if let Ok(Some(cached_embedding)) = redis_manager.get_cached_embedding(text).await {
        info!("Using cached embedding for text: {}", text);
        return Ok(cached_embedding);
    }

    info!("Generating new OpenAI embedding for text: {}", text);

    let config = OpenAIConfig::new().with_api_key(openai_api_key.to_string());
    let client = Client::with_config(config);
    let request = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-3-small".to_string())
        .input(EmbeddingInput::String(text.to_string()))
        .build()?;

    let response = client.embeddings().create(request).await?;

    let embedding = response.data.into_iter().next()
        .ok_or_else(|| UnifiedIntelligenceError::Other(anyhow::anyhow!("No embeddings returned")))?
        .embedding;

    // Cache the embedding for 7 days
    if let Err(e) = redis_manager.set_cached_embedding(text, &embedding, 86400 * 7).await {
        warn!("Failed to cache embedding: {}", e);
    }

    Ok(embedding)
}
