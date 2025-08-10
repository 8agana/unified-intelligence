use crate::config::Config;
use crate::redis::RedisManager;
use anyhow::{Context, Result, anyhow};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
};
use bytemuck;
use chrono::Utc;
use redis::AsyncCommands;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct UiMemoryParams {
    pub action: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_scope")]
    pub scope: Option<String>,
    #[serde(default)]
    pub filters: Option<MemoryFilters>,
    #[serde(default)]
    pub options: Option<MemoryOptions>,
    #[serde(default)]
    pub targets: Option<MemoryTargets>,
    #[serde(default)]
    pub update: Option<MemoryUpdate>,
}

fn default_scope() -> Option<String> {
    Some("all".to_string())
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct MemoryFilters {
    #[serde(default)]
    pub tags: Vec<String>,
    pub importance: Option<String>,
    pub chain_id: Option<String>,
    pub thought_id: Option<String>,
    pub time_range: Option<MemoryTimeRange>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct MemoryTimeRange {
    pub after: Option<String>,
    pub before: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct MemoryOptions {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_k")]
    pub k: u32,
    #[serde(default = "default_search_type")]
    pub search_type: String,
    pub min_score: Option<f32>,
    pub ef_runtime: Option<u32>,
}

impl Default for MemoryOptions {
    fn default() -> Self {
        Self {
            limit: default_limit(),
            offset: 0,
            k: default_k(),
            search_type: default_search_type(),
            min_score: None,
            ef_runtime: None,
        }
    }
}

fn default_limit() -> u32 {
    10
}
fn default_k() -> u32 {
    10
}
fn default_search_type() -> String {
    "hybrid".to_string()
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct MemoryTargets {
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct MemoryUpdate {
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub importance: Option<String>,
    pub chain_id: Option<String>,
    pub thought_id: Option<String>,
    pub ttl_seconds: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct MemoryItem {
    pub key: String,
    pub content: String,
    pub tags: Vec<String>,
    pub importance: String,
    pub chain_id: String,
    pub thought_id: String,
    pub ts: i64,
    pub score: Option<f32>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
pub struct UiMemoryResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<MemoryItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn short_hash(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8])
}

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
fn determine_indexes(instance_id: &str, scope: &str) -> Vec<String> {
    let mut indexes = Vec::new();
    match scope {
        "session-summaries" => indexes.push(format!("idx:{instance_id}:session-summaries")),
        "important" => indexes.push(format!("idx:{instance_id}:important")),
        "federation" => indexes.push("idx:Federation:embeddings".to_string()),
        "all" => {
            indexes.push(format!("idx:{instance_id}:session-summaries"));
            indexes.push(format!("idx:{instance_id}:important"));
            indexes.push("idx:Federation:embeddings".to_string());
        }
        _ => {}
    }
    indexes
}

fn parse_key_scope(key: &str) -> (String, String) {
    let parts: Vec<&str> = key.split(':').collect();
    if parts.len() > 2 {
        (parts[0].to_string(), parts[2].to_string())
    } else {
        ("unknown".to_string(), "unknown".to_string())
    }
}

pub async fn ui_memory_impl(
    config: &Config,
    redis_manager: &RedisManager,
    params: UiMemoryParams,
) -> Result<UiMemoryResult> {
    let mut con = redis_manager.get_connection().await?;

    match params.action.as_str() {
        "search" => {
            // Keyword-only search MVP with filters. Uses RediSearch to get ids, then HGET fields.
            let instance_id = std::env::var("INSTANCE_ID")
                .unwrap_or_else(|_| config.server.default_instance_id.clone());
            let scope = params.scope.as_deref().unwrap_or("all");
            let indexes = determine_indexes(&instance_id, scope);
            let options = params.options.clone().unwrap_or_default();

            let mut query = String::new();
            if let Some(q) = params.query.as_deref() {
                if !q.trim().is_empty() {
                    query.push_str(q.trim());
                }
            }
            if let Some(f) = params.filters.as_ref() {
                if !f.tags.is_empty() {
                    if !query.is_empty() { query.push(' '); }
                    let tags = f.tags.join("|");
                    query.push_str(&format!("@tags:{{{tags}}}"));
                }
                if let Some(imp) = &f.importance {
                    if !query.is_empty() { query.push(' '); }
                    query.push_str(&format!("@importance:{imp}"));
                }
                if let Some(cid) = &f.chain_id {
                    if !query.is_empty() { query.push(' '); }
                    query.push_str(&format!("@chain_id:{cid}"));
                }
                if let Some(tid) = &f.thought_id {
                    if !query.is_empty() { query.push(' '); }
                    query.push_str(&format!("@thought_id:{tid}"));
                }
            }
            if query.is_empty() { query.push('*'); }

            let mut all_items: Vec<MemoryItem> = Vec::new();
            for idx in indexes {
                let res: redis::Value = redis::cmd("FT.SEARCH")
                    .arg(&idx)
                    .arg(&query)
                    .arg("NOCONTENT")
                    .arg("LIMIT").arg(options.offset).arg(options.limit)
                    .query_async(&mut *con)
                    .await?;
                let keys = extract_doc_ids(&res);
                if keys.is_empty() { continue; }

                let mut pipe = redis::pipe();
                for k in &keys { pipe.hgetall(k); }
                let maps: Vec<HashMap<String, String>> = pipe.query_async(&mut *con).await?;
                for (i, m) in maps.into_iter().enumerate() {
                    if m.is_empty() { continue; }
                    all_items.push(MemoryItem{
                        key: keys[i].clone(),
                        content: m.get("content").cloned().unwrap_or_default(),
                        tags: m.get("tags").map(|s| s.split(',').map(|x| x.to_string()).collect()).unwrap_or_default(),
                        importance: m.get("importance").cloned().unwrap_or_default(),
                        chain_id: m.get("chain_id").cloned().unwrap_or_default(),
                        thought_id: m.get("thought_id").cloned().unwrap_or_default(),
                        ts: m.get("ts").and_then(|s| s.parse().ok()).unwrap_or_default(),
                        score: None,
                    });
                }
            }
            Ok(UiMemoryResult { results: Some(all_items), ..Default::default() })
        }
        "read" => {
            let keys = params.targets.context("Missing targets for read")?.keys;
            if keys.is_empty() {
                return Ok(UiMemoryResult {
                    results: Some(vec![]),
                    ..Default::default()
                });
            }

            let mut pipe = redis::pipe();
            for key in &keys {
                pipe.hgetall(key);
            }
            let results: Vec<HashMap<String, String>> = pipe.query_async(&mut *con).await?;

            let mut memory_items = Vec::new();
            for (i, data) in results.iter().enumerate() {
                if !data.is_empty() {
                    memory_items.push(MemoryItem {
                        key: keys[i].clone(),
                        content: data.get("content").cloned().unwrap_or_default(),
                        tags: data
                            .get("tags")
                            .map(|s| s.split(',').map(String::from).collect())
                            .unwrap_or_default(),
                        importance: data.get("importance").cloned().unwrap_or_default(),
                        chain_id: data.get("chain_id").cloned().unwrap_or_default(),
                        thought_id: data.get("thought_id").cloned().unwrap_or_default(),
                        ts: data
                            .get("ts")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_default(),
                        score: None,
                    });
                }
            }
            Ok(UiMemoryResult {
                results: Some(memory_items),
                ..Default::default()
            })
        }
        "delete" => {
            let keys = params.targets.context("Missing targets for delete")?.keys;
            if keys.is_empty() {
                return Ok(UiMemoryResult {
                    deleted: Some(0),
                    ..Default::default()
                });
            }
            let count: usize = con.del(&keys).await?;
            Ok(UiMemoryResult {
                deleted: Some(count),
                ..Default::default()
            })
        }
        "update" => {
            let keys = params.targets.context("Missing targets for update")?.keys;
            let update_data = params.update.context("Missing update data")?;
            let mut updated_pairs = Vec::new();

            for key in &keys {
                if let Some(content) = &update_data.content {
                    let (instance, scope) = parse_key_scope(key);
                    let new_hash = short_hash(content);
                    let new_key = match scope.as_str() {
                        "session-summaries" | "important" => {
                            format!("{instance}:embeddings:{scope}:{new_hash}")
                        }
                        _ => format!("Federation:embeddings:{new_hash}"),
                    };

                    // Re-embed
                    let vector_f32 = openai_embed(config, content).await?;
                    let dims = config.openai.embedding_dimensions;
                    if vector_f32.len() != dims {
                        return Err(anyhow!("embedding dims mismatch"));
                    }
                    let vector_bytes: Vec<u8> = bytemuck::cast_slice(&vector_f32).to_vec();
                    let ts = Utc::now().timestamp();

                    let mut pipe = redis::pipe();
                    pipe.hset(&new_key, "content", content)
                        .hset(&new_key, "ts", ts)
                        .hset(&new_key, "vector", vector_bytes);
                    if let Some(tags) = &update_data.tags {
                        pipe.hset(&new_key, "tags", tags.join(","));
                    }
                    if let Some(imp) = &update_data.importance {
                        pipe.hset(&new_key, "importance", imp);
                    }
                    if let Some(cid) = &update_data.chain_id {
                        pipe.hset(&new_key, "chain_id", cid);
                    }
                    if let Some(tid) = &update_data.thought_id {
                        pipe.hset(&new_key, "thought_id", tid);
                    }
                    if let Some(ttl) = update_data.ttl_seconds {
                        if ttl > 0 {
                            pipe.expire(&new_key, ttl as i64);
                        }
                    }
                    pipe.del(key);
                    let _: () = pipe.query_async(&mut *con).await?;
                    updated_pairs.push((key.clone(), new_key));
                } else {
                    let mut pipe = redis::pipe();
                    let mut has_update = false;
                    if let Some(tags) = &update_data.tags {
                        pipe.hset(key, "tags", tags.join(","));
                        has_update = true;
                    }
                    if let Some(importance) = &update_data.importance {
                        pipe.hset(key, "importance", importance);
                        has_update = true;
                    }
                    // ... other fields
                    if has_update {
                        let _: () = pipe.query_async(&mut *con).await?;
                    }
                    if let Some(ttl) = update_data.ttl_seconds {
                        if ttl > 0 {
                            let _: () = con.expire(key, ttl as i64).await?;
                        }
                    }
                    updated_pairs.push((key.clone(), key.clone()));
                }
            }
            Ok(UiMemoryResult {
                updated: Some(updated_pairs),
                ..Default::default()
            })
        }
        _ => Ok(UiMemoryResult {
            message: Some(format!("Unknown action: {}", params.action)),
            ..Default::default()
        }),
    }
}

fn extract_doc_ids(val: &redis::Value) -> Vec<String> {
    // RediSearch NOCONTENT response (RESP3): [total(Int), id1(BulkString), id2(BulkString), ...]
    let mut out = Vec::new();
    match val {
        redis::Value::Array(items) if !items.is_empty() => {
            for item in items.iter().skip(1) {
                match item {
                    redis::Value::BulkString(bytes) => {
                        if let Ok(s) = std::str::from_utf8(bytes) {
                            out.push(s.to_string());
                        }
                    }
                    redis::Value::SimpleString(s) => out.push(s.clone()),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    out
}
