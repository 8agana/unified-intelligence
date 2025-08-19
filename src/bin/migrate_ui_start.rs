use anyhow::Result;
use bytemuck::cast_slice;
use redis::AsyncCommands;
use serde_json::Value;
use std::sync::Arc;

use unified_intelligence::config::Config;
use unified_intelligence::redis::RedisManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Minimal stderr tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    let config = Arc::new(Config::load());
    let redis = RedisManager::new_with_config(&config).await?;
    let instance_id =
        std::env::var("INSTANCE_ID").unwrap_or_else(|_| config.server.default_instance_id.clone());

    // Ensure target index exists
    let dims = config.openai.embedding_dimensions;
    let index = format!("idx:{instance_id}:session-summaries");
    let prefix = format!("{instance_id}:embeddings:session-summaries:");
    let _ = ensure_index_hash_hnsw(
        &redis,
        &index,
        &prefix,
        dims,
        config.redis_search.hnsw.m,
        config.redis_search.hnsw.ef_construction,
    )
    .await;

    let mut con = redis.get_connection().await?;
    let pattern = format!("{instance_id}:ui_start:emb:*");
    let mut cursor: u64 = 0;
    let mut migrated: usize = 0;
    let mut skipped: usize = 0;
    let chunk_size: usize = 2000;

    loop {
        // SCAN for raw ui_start embedding keys
        let scan_res: (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(500)
            .query_async(&mut *con)
            .await?;
        cursor = scan_res.0;
        for key in scan_res.1 {
            // key format: {instance}:ui_start:emb:{chain}:{offset}
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() < 5 {
                tracing::warn!("Skipping unexpected key format: {}", key);
                skipped += 1;
                continue;
            }
            let chain_id = parts[3];
            let offset_str = parts[4];
            let start: usize = offset_str.parse().unwrap_or(0);

            let target_key =
                format!("{instance_id}:embeddings:session-summaries:{chain_id}:{start}");
            // If already migrated, skip
            let exists: i32 = redis::cmd("EXISTS")
                .arg(&target_key)
                .query_async(&mut *con)
                .await?;
            if exists == 1 {
                skipped += 1;
                continue;
            }

            // Fetch vector bytes from old key (bincode-serialized Vec<f32>)
            let raw: Option<Vec<u8>> = con.get(&key).await?;
            let Some(raw) = raw else {
                tracing::warn!("No data at key {}, skipping", key);
                skipped += 1;
                continue;
            };
            let vec_f32: Vec<f32> = match bincode::deserialize(&raw) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to deserialize embedding at {}: {}", key, e);
                    skipped += 1;
                    continue;
                }
            };
            let vec_bytes: Vec<u8> = cast_slice(&vec_f32).to_vec();

            // Load the corresponding summary JSON to reconstruct chunk content
            let sum_key = format!("{instance_id}:ui_start:summary:{chain_id}");
            let json: Option<String> = redis::cmd("JSON.GET")
                .arg(&sum_key)
                .arg("$")
                .query_async(&mut *con)
                .await?;
            let mut chunk = String::new();
            let mut ts: i64 = chrono::Utc::now().timestamp();
            if let Some(js) = json {
                // ui_start stored root object, JSON.GET $ returns [obj]
                if let Ok(mut arr) = serde_json::from_str::<Vec<Value>>(&js) {
                    if let Some(obj) = arr.pop() {
                        if let Some(s) = obj.get("summary").and_then(Value::as_str) {
                            let bytes = s.as_bytes();
                            let end = std::cmp::min(start + chunk_size, bytes.len());
                            // Adjust to char boundaries to avoid panic
                            let mut ss = start;
                            let mut ee = end;
                            while ss < s.len() && !s.is_char_boundary(ss) {
                                ss += 1;
                            }
                            while ee > ss && !s.is_char_boundary(ee) {
                                ee -= 1;
                            }
                            chunk = s.get(ss..ee).unwrap_or("").to_string();
                        }
                        if let Some(created) = obj.get("created_at").and_then(Value::as_str) {
                            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created) {
                                ts = dt.timestamp();
                            }
                        }
                    }
                }
            }

            // Upsert HASH doc to target prefix
            let tags_csv = "session-summary";
            let _: () = redis::pipe()
                .hset(&target_key, "content", &chunk)
                .hset(&target_key, "tags", tags_csv)
                .hset(&target_key, "category", "session-summary")
                .hset(&target_key, "importance", "")
                .hset(&target_key, "chain_id", chain_id)
                .hset(&target_key, "thought_id", "")
                .hset(&target_key, "ts", ts)
                .hset(&target_key, "vector", vec_bytes)
                .del(&key) // remove old raw SET key
                .query_async(&mut *con)
                .await?;
            migrated += 1;
        }
        if cursor == 0 {
            break;
        }
    }

    eprintln!("Migration complete. migrated={migrated}, skipped={skipped} (existing/invalid)");
    Ok(())
}

async fn ensure_index_hash_hnsw(
    redis_manager: &RedisManager,
    index: &str,
    prefix: &str,
    dims: usize,
    m: u32,
    ef_construction: u32,
) -> std::result::Result<bool, redis::RedisError> {
    let mut con = redis_manager.get_connection().await.map_err(|e| match e {
        unified_intelligence::error::UnifiedIntelligenceError::Redis(e) => e,
        other => redis::RedisError::from(std::io::Error::other(other.to_string())),
    })?;

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
            } else {
                Err(e)
            }
        }
    }
}
