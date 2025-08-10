use anyhow::{Result, anyhow, ensure};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
};
use bytemuck;
use chrono::Utc;
use hex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct UIContextParams {
    /// "session-summaries" | "important" | "federation" | "help"
    #[serde(rename = "type")]
    pub kind: String,

    /// Required unless type == "help"
    #[serde(default)]
    pub content: String,

    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub importance: Option<String>,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub thought_id: Option<String>,

    /// Optional override; else cfg.server.default_instance_id
    #[serde(default)]
    pub instance_id: Option<String>,

    /// Seconds; else cfg.redis.default_ttl_seconds
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct UIContextResult {
    pub mode: String,
    pub key: Option<String>,
    pub index: Option<String>,
    pub dims: Option<usize>,
    pub upserted: bool,
    pub created_index: bool,
    pub message: Option<String>,
}

// This function will be called from UnifiedIntelligenceService
// The actual tool implementation should be added to service.rs
#[allow(dead_code)]
pub async fn ui_context_impl(
    config: &Config,
    redis_manager: &crate::redis::RedisManager,
    params: UIContextParams,
) -> Result<UIContextResult> {
    let mode = params.kind.trim();

    // help (no external calls)
    if mode.eq_ignore_ascii_case("help") {
        return Ok(UIContextResult {
            mode: "help".into(),
            key: None,
            index: None,
            dims: None,
            upserted: false,
            created_index: false,
            message: Some(help_text()),
        });
    }

    // only the three write modes are allowed
    ensure!(
        matches!(mode, "session-summaries" | "important" | "federation"),
        "unsupported type: {} (expected: session-summaries|important|federation|help)",
        mode
    );
    ensure!(
        !params.content.is_empty(),
        "`content` is required for type {}",
        mode
    );

    let instance_id = params
        .instance_id
        .clone()
        .unwrap_or_else(|| config.server.default_instance_id.clone());

    // --- Embedding ---
    let dims = config.openai.embedding_dimensions; // expect 1536 for text-embedding-3-small
    let vector_f32 = openai_embed(config, &params.content).await?;
    ensure!(
        vector_f32.len() == dims,
        "embedding dims mismatch: got {}, expected {}",
        vector_f32.len(),
        dims
    );

    // --- Key / Index ---
    let (key, index, prefix) = match mode {
        "session-summaries" => {
            let id = short_hash(&params.content);
            (
                format!("{instance_id}:embeddings:session-summaries:{id}"),
                format!("idx:{instance_id}:session-summaries"),
                format!("{instance_id}:embeddings:session-summaries:"),
            )
        }
        "important" => {
            let id = short_hash(&params.content);
            (
                format!("{instance_id}:embeddings:important:{id}"),
                format!("idx:{instance_id}:important"),
                format!("{instance_id}:embeddings:important:"),
            )
        }
        "federation" => {
            let id = short_hash(&params.content);
            (
                format!("Federation:embeddings:{id}"),
                "idx:Federation:embeddings".to_string(),
                "Federation:embeddings:".to_string(),
            )
        }
        _ => unreachable!(),
    };

    // --- Ensure RediSearch index exists ---
    let created_index = ensure_index_if_needed(
        redis_manager,
        &index,
        &prefix,
        dims,
        config.redis_search.hnsw.m,
        config.redis_search.hnsw.ef_construction,
    )
    .await
    .map_err(|e| anyhow!("ensure index {index}: {e}"))?;

    // --- Write HASH + TTL ---
    // Convert f32 vector to raw bytes for RediSearch VECTOR field
    let vector_bytes: Vec<u8> = bytemuck::cast_slice(&vector_f32).to_vec();
    let ts = Utc::now().timestamp();
    let tags_csv = if params.tags.is_empty() {
        "".into()
    } else {
        params.tags.join(",")
    };

    {
        let mut con = redis_manager.get_connection().await?;
        let mut pipe = redis::pipe();

        pipe.hset(&key, "content", &params.content)
            .hset(&key, "tags", tags_csv)
            .hset(&key, "importance", params.importance.unwrap_or_default())
            .hset(&key, "chain_id", params.chain_id.unwrap_or_default())
            .hset(&key, "thought_id", params.thought_id.unwrap_or_default())
            .hset(&key, "ts", ts)
            .hset(&key, "vector", vector_bytes);

        if let Some(ttl) = params.ttl_seconds {
            if ttl > 0 {
                pipe.expire(&key, ttl as i64);
            }
        }

        let _: () = pipe.query_async(&mut *con).await?;
    }

    Ok(UIContextResult {
        mode: mode.into(),
        key: Some(key),
        index: Some(index),
        dims: Some(dims),
        upserted: true,
        created_index,
        message: None,
    })
}

// ---- helpers ----

#[allow(dead_code)]
fn help_text() -> String {
    r#"ui_context help:
- type="session-summaries": subagent/LLM session synthesis → {instance}:embeddings:session-summaries:{id}
- type="important":        long-running local context → {instance}:embeddings:important:{id}
- type="federation":       federation-wide context → Federation:embeddings:{id}
Required: content (except for help).
Optional: tags[], importance, chain_id, thought_id, ttl_seconds, instance_id."#.into()
}

#[allow(dead_code)]
async fn openai_embed(cfg: &Config, text: &str) -> Result<Vec<f32>> {
    let config = OpenAIConfig::new().with_api_key(cfg.openai.api_key()?);
    let client = Client::with_config(config);
    let req = CreateEmbeddingRequestArgs::default()
        .model(cfg.openai.embedding_model.clone())
        .input(EmbeddingInput::String(text.to_owned()))
        .build()?;
    let resp = client.embeddings().create(req).await?;
    let first = resp
        .data
        .first()
        .ok_or_else(|| anyhow!("no embedding returned"))?;
    Ok(first.embedding.clone())
}

#[allow(dead_code)]
fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let hex = hex::encode(h.finalize());
    hex[..16].to_string()
}

#[allow(dead_code)]
async fn ensure_index_if_needed(
    redis_manager: &crate::redis::RedisManager,
    index: &str,
    prefix: &str,
    dims: usize,
    m: u32,
    ef_construction: u32,
) -> Result<bool> {
    let mut con = redis_manager.get_connection().await?;

    // Check if index exists (response is a complex structure; we only care about success)
    let info: redis::RedisResult<redis::Value> = redis::cmd("FT.INFO")
        .arg(index)
        .query_async(&mut *con)
        .await;

    if let Err(e) = &info {
        // If RediSearch is not loaded, FT.INFO will be an unknown command.
        if e.to_string().to_lowercase().contains("unknown command")
            || e.to_string().to_lowercase().contains("module not loaded")
        {
            return Err(anyhow!(
                "RediSearch not available: {}. Ensure the RediSearch module is loaded.",
                e
            ));
        }
    }

    if info.is_ok() {
        return Ok(false);
    }

    // Create index
    // HNSW requires the count of subsequent arguments (key-value pairs).
    // We pass 5 pairs => 10 arguments.
    let create_res: redis::RedisResult<()> = redis::cmd("FT.CREATE")
        .arg(index)
        .arg("ON")
        .arg("HASH")
        .arg("PREFIX")
        .arg(1)
        .arg(prefix)
        .arg("SCHEMA")
        .arg("content")
        .arg("TEXT")
        .arg("tags")
        .arg("TAG")
        .arg("SEPARATOR")
        .arg(",")
        .arg("importance")
        .arg("TEXT")
        .arg("ts")
        .arg("NUMERIC")
        .arg("SORTABLE")
        .arg("vector")
        .arg("VECTOR")
        .arg("HNSW")
        .arg(10)
        .arg("TYPE")
        .arg("FLOAT32")
        .arg("DIM")
        .arg(dims)
        .arg("DISTANCE_METRIC")
        .arg("COSINE")
        .arg("M")
        .arg(m)
        .arg("EF_CONSTRUCTION")
        .arg(ef_construction)
        .query_async(&mut *con)
        .await;

    match create_res {
        Ok(()) => Ok(true),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("index already exists") {
                Ok(false)
            } else if msg.contains("unknown command") || msg.contains("module not loaded") {
                Err(anyhow!(
                    "RediSearch not available: {}. Ensure the RediSearch module is loaded.",
                    e
                ))
            } else {
                Err(anyhow!("FT.CREATE {} failed: {}", index, e))
            }
        }
    }
}
