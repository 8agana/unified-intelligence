use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
};
use bytemuck::cast_slice;

// Minimal structs matching stored JSON
#[derive(serde::Deserialize)]
struct ThoughtRecord {
    id: String,
    thought: String,
    chain_id: Option<String>,
    importance: Option<i32>,
    tags: Option<Vec<String>>,
    category: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum KnowledgeScope {
    Federation,
    Personal,
}
impl std::fmt::Display for KnowledgeScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeScope::Federation => write!(f, "Federation"),
            KnowledgeScope::Personal => write!(f, "Personal"),
        }
    }
}

#[derive(serde::Deserialize)]
struct KnowledgeNode {
    id: String,
    display_name: String,
    attributes: std::collections::HashMap<String, serde_json::Value>,
    tags: Vec<String>,
}

// Lightweight config and Redis manager for this tool
struct SimpleConfig {
    instance_id: String,
    dims: usize,
    hnsw_m: u32,
    hnsw_ef: u32,
    redis_url: String,
}

impl SimpleConfig {
    fn load() -> Self {
        let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "CC".to_string());
        let dims = std::env::var("OPENAI_EMBED_DIMS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1536);
        let hnsw_m = std::env::var("REDIS_HNSW_M")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);
        let hnsw_ef = std::env::var("REDIS_HNSW_EF_CONSTRUCTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        // Build Redis URL with auth if password is provided
        let redis_url = if let Ok(password) = std::env::var("REDIS_PASSWORD") {
            let base_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string());
            // Insert password into URL if not already present
            if base_url.starts_with("redis://") && !base_url.contains('@') {
                base_url.replacen("redis://", &format!("redis://:{password}@"), 1)
            } else {
                base_url
            }
        } else {
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string())
        };
        Self {
            instance_id,
            dims,
            hnsw_m,
            hnsw_ef,
            redis_url,
        }
    }
}

struct RedisManager {
    pool: deadpool_redis::Pool,
}
impl RedisManager {
    async fn new(url: &str) -> anyhow::Result<Self> {
        let cfg = deadpool_redis::Config::from_url(url);
        let pool = cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?;
        Ok(Self { pool })
    }
    async fn get_connection(&self) -> anyhow::Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }
}

async fn generate_openai_embedding(text: &str, api_key: &str) -> anyhow::Result<Vec<f32>> {
    let cfg = OpenAIConfig::new().with_api_key(api_key.to_string());
    let client = Client::with_config(cfg);
    let req = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-3-small".to_string())
        .input(EmbeddingInput::String(text.to_string()))
        .build()?;
    let resp = client.embeddings().create(req).await?;
    let embedding = resp
        .data
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No embeddings returned"))?
        .embedding;
    Ok(embedding)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = Arc::new(SimpleConfig::load());
    let instance_id = cfg.instance_id.clone();

    let openai_key =
        std::env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY is not set"))?;

    let redis = Arc::new(RedisManager::new(&cfg.redis_url).await?);
    let dims = cfg.dims;

    // Ensure indices exist for thoughts and KG entities
let thought_index = format!("idx:{instance_id}:thought");
let thought_prefix = format!("{instance_id}:embeddings:thought:");
    ensure_index_hash_hnsw(
        &redis,
        &thought_index,
        &thought_prefix,
        dims,
        cfg.hnsw_m,
        cfg.hnsw_ef,
    )
    .await?;

let kg_index = format!("idx:{instance_id}:kg_entity");
let kg_prefix = format!("{instance_id}:embeddings:kg_entity:");
    ensure_index_hash_hnsw(&redis, &kg_index, &kg_prefix, dims, cfg.hnsw_m, cfg.hnsw_ef).await?;

    // Backfill thoughts -> embeddings
    let thoughts_count = backfill_thoughts(&redis, &openai_key, &instance_id, dims).await?;
    tracing::info!("Backfilled {} thought embeddings", thoughts_count);

    // Backfill KG entities -> embeddings
    let kg_count = backfill_kg_entities(&redis, &openai_key, &instance_id, dims).await?;
    tracing::info!("Backfilled {} KG entity embeddings", kg_count);

    println!(
        "Backfill complete: thoughts={thoughts_count}, kg_entities={kg_count}"
    );
    Ok(())
}

async fn backfill_thoughts(
    redis: &RedisManager,
    openai_key: &str,
    instance_id: &str,
    dims: usize,
) -> Result<usize> {
    let mut conn = redis.get_connection().await?;
let pattern = format!("{instance_id}:Thoughts:*");
    let mut cursor: u64 = 0;
    let mut processed = 0usize;

    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(200)
            .query_async(&mut *conn)
            .await
            .context("SCAN Thoughts failed")?;

        for key in keys {
            // Get ThoughtRecord JSON (RedisJSON returns array when path is $)
            let json_opt: Option<String> = redis::cmd("JSON.GET")
                .arg(&key)
                .arg("$")
                .query_async(&mut *conn)
                .await
                .ok();
            let Some(json_str) = json_opt else {
                continue;
            };
            let records: Vec<ThoughtRecord> = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(rec) = records.into_iter().next() else {
                continue;
            };

            // Skip empty or very short content
            if rec.thought.trim().is_empty() {
                continue;
            }

            // Embed
let emb = match generate_openai_embedding(&rec.thought, openai_key).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Embedding failed for thought {}: {}", rec.id, e);
                    continue;
                }
            };
            if emb.len() != dims {
                tracing::warn!("Unexpected dims for thought {}", rec.id);
                continue;
            }
            let vec_bytes: Vec<u8> = cast_slice(&emb).to_vec();

            // Store HASH doc for RediSearch
            let doc_key = format!("{}:embeddings:thought:{}", instance_id, rec.id);
            let ts = chrono::Utc::now().timestamp();
            let tags_csv = rec.tags.as_ref().map(|v| v.join(",")).unwrap_or_default();

            let _: () = redis::pipe()
                .hset(&doc_key, "content", &rec.thought)
                .hset(&doc_key, "tags", tags_csv)
                .hset(
                    &doc_key,
                    "category",
                    rec.category.clone().unwrap_or_default(),
                )
                .hset(
                    &doc_key,
                    "importance",
                    rec.importance.unwrap_or(5).to_string(),
                )
                .hset(
                    &doc_key,
                    "chain_id",
                    rec.chain_id.clone().unwrap_or_default(),
                )
                .hset(&doc_key, "thought_id", &rec.id)
                .hset(&doc_key, "ts", ts)
                .hset(&doc_key, "vector", vec_bytes)
                .query_async(&mut *conn)
                .await
                .unwrap_or(());

            processed += 1;
        }

        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(processed)
}

async fn backfill_kg_entities(
    redis: &RedisManager,
    openai_key: &str,
    instance_id: &str,
    dims: usize,
) -> Result<usize> {
    let mut conn = redis.get_connection().await?;

    // Personal scope entities are stored under {instance}:KG:entity:* ; federation under literal scope name
let personal_pattern = format!("{instance_id}:KG:entity:*");
let federation_pattern = format!("{}:KG:entity:*", KnowledgeScope::Federation);

let mut total = 0usize;
total += scan_and_embed_entities(
        &mut conn,
        &personal_pattern,
        openai_key,
        instance_id,
        dims,
    )
    .await?;
    total += scan_and_embed_entities(
        &mut conn,
        &federation_pattern,
        openai_key,
        instance_id,
        dims,
    )
    .await?;
    Ok(total)
}

async fn scan_and_embed_entities(
    conn: &mut deadpool_redis::Connection,
    pattern: &str,
    openai_key: &str,
    instance_id: &str,
    dims: usize,
) -> Result<usize> {
    let mut cursor: u64 = 0;
    let mut processed = 0usize;

    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(200)
            .query_async(&mut **conn)
            .await
            .context("SCAN KG entities failed")?;

        for key in keys {
            let json_opt: Option<String> = redis::cmd("JSON.GET")
                .arg(&key)
                .arg("$")
                .query_async(&mut **conn)
                .await
                .ok();
            let Some(json_str) = json_opt else {
                continue;
            };
            let nodes: Vec<KnowledgeNode> = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(node) = nodes.into_iter().next() else {
                continue;
            };

            // Build compact text for embedding
            let mut text = node.display_name.clone();
            if !node.tags.is_empty() {
                text.push_str(" | tags: ");
                text.push_str(&node.tags.join(", "));
            }
            if !node.attributes.is_empty() {
                let snapshot = serde_json::to_string(&node.attributes).unwrap_or_default();
                // truncate long attributes snapshot
                let snap = snapshot.chars().take(400).collect::<String>();
                text.push_str(" | attrs: ");
                text.push_str(&snap);
            }

            if text.trim().is_empty() {
                continue;
            }

let emb = match generate_openai_embedding(&text, openai_key).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Embedding failed for entity {}: {}", node.id, e);
                    continue;
                }
            };
            if emb.len() != dims {
                tracing::warn!("Unexpected dims for entity {}", node.id);
                continue;
            }
            let vec_bytes: Vec<u8> = cast_slice(&emb).to_vec();

            let doc_key = format!("{}:embeddings:kg_entity:{}", instance_id, node.id);
            let ts = chrono::Utc::now().timestamp();
            let tags_csv = if node.tags.is_empty() {
                String::new()
            } else {
                node.tags.join(",")
            };

            let _: () = redis::pipe()
                .hset(&doc_key, "content", &text)
                .hset(&doc_key, "tags", tags_csv)
                .hset(&doc_key, "category", "kg_entity")
                .hset(&doc_key, "importance", "")
                .hset(&doc_key, "chain_id", "")
                .hset(&doc_key, "thought_id", "")
                .hset(&doc_key, "ts", ts)
                .hset(&doc_key, "vector", vec_bytes)
                .query_async(&mut **conn)
                .await
                .unwrap_or(());

            processed += 1;
        }

        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(processed)
}

// Ensure an HNSW RediSearch index exists for HASH prefixes (copy of service helper)
async fn ensure_index_hash_hnsw(
    redis_manager: &RedisManager,
    index: &str,
    prefix: &str,
    dims: usize,
    m: u32,
    ef_construction: u32,
) -> Result<bool> {
    let mut con = redis_manager.get_connection().await?;

    let info: redis::RedisResult<redis::Value> = redis::cmd("FT.INFO")
        .arg(index)
        .query_async(&mut *con)
        .await;
    if info.is_ok() {
        return Ok(false);
    }

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
        .arg("category")
        .arg("TEXT")
        .arg("importance")
        .arg("TEXT")
        .arg("chain_id")
        .arg("TEXT")
        .arg("thought_id")
        .arg("TEXT")
        .arg("ts")
        .arg("NUMERIC")
        .arg("SORTABLE")
        .arg("vector")
        .arg("VECTOR")
        .arg("HNSW")
        .arg(10)  // number of parameters
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

    create_res.map(|_| true).map_err(|e| e.into())
}
