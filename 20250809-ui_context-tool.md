⸻

0) Preconditions (fail fast)
	1.	A running Redis server with the RediSearch module loaded.
	•	Quick check:
	•	redis-cli MODULE LIST | grep -i search → must show a search module.
	•	redis-cli FT._LIST → should not error.
	•	If FT.INFO says “unknown command,” RediSearch is not loaded. Stop.
	2.	Project already uses rmcp 0.5.x with a ToolRouter type annotated via #[tool_router].

⸻

1) Add/confirm dependencies

Run at repo root:

cargo add rmcp@0.5 rmcp-macros@0.5
cargo add async-openai bytemuck deadpool-redis redis schemars sha2 hex chrono anyhow
cargo add serde --features derive
cargo add serde_json

Ensure redis has Tokio support in Cargo.toml:

redis = { version = "*", features = ["tokio-comp"] }


⸻

2) Config keys (don’t remove your existing values)

Make sure these exist (names can map to your current config loader):

openai:
  embedding_model: text-embedding-3-small
  embedding_dimensions: 1536

server:
  default_instance_id: CC

redis:
  default_ttl_seconds: 604800  # 7 days

redis_search:
  hnsw:
    m: 16
    ef_construction: 200


⸻

3) Create file: src/tools/ui_context.rs

Create the directory if missing and paste this entire file.

use anyhow::{anyhow, ensure, Context, Result};
use async_openai::{Client as OAClient, types::{CreateEmbeddingRequest, EmbeddingInput}};
use bytemuck;
use chrono::Utc;
use deadpool_redis::Pool;
use redis::AsyncCommands;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::router::ToolRouter; // Your router struct annotated with #[tool_router]

#[derive(Deserialize, JsonSchema)]
pub struct UIContextParams {
    /// "session-summaries" | "important" | "federation" | "help"
    #[serde(rename = "type")]
    pub kind: String,

    /// Required unless type == "help"
    #[serde(default)]
    pub content: String,

    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub importance: Option<String>,
    #[serde(default)] pub chain_id: Option<String>,
    #[serde(default)] pub thought_id: Option<String>,

    /// Optional override; else cfg.server.default_instance_id
    #[serde(default)] pub instance_id: Option<String>,

    /// Seconds; else cfg.redis.default_ttl_seconds
    #[serde(default)] pub ttl_seconds: Option<u64>,
}

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

#[rmcp_macros::tool_handler]
impl ToolRouter {
    /// ui_context:
    /// - type="session-summaries": store session synthesis under {instance}:embeddings:session-summaries:{id}
    /// - type="important": store long-running local context under {instance}:embeddings:important:{id}
    /// - type="federation": store federation-wide context under Federation:embeddings:{id}
    /// - type="help": returns usage text; writes nothing
    pub async fn ui_context(&self, p: UIContextParams) -> Result<UIContextResult> {
        let mode = p.kind.trim();

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
        ensure!(!p.content.is_empty(), "`content` is required for type {}", mode);

        let cfg: &Config = &self.cfg;
        let instance_id = p
            .instance_id
            .clone()
            .unwrap_or_else(|| cfg.server.default_instance_id.clone());

        // --- Embedding ---
        let dims = cfg.openai.embedding_dimensions as usize; // expect 1536 for text-embedding-3-small
        let vector_f32 = openai_embed(cfg, &p.content).await?;
        ensure!(
            vector_f32.len() == dims,
            "embedding dims mismatch: got {}, expected {}",
            vector_f32.len(),
            dims
        );

        // --- Key / Index ---
        let (key, index, prefix) = match mode {
            "session-summaries" => {
                let id = short_hash(&p.content);
                (
                    format!("{instance_id}:embeddings:session-summaries:{id}"),
                    format!("idx:{instance_id}:session-summaries"),
                    format!("{instance_id}:embeddings:session-summaries:"),
                )
            }
            "important" => {
                let id = short_hash(&p.content);
                (
                    format!("{instance_id}:embeddings:important:{id}"),
                    format!("idx:{instance_id}:important"),
                    format!("{instance_id}:embeddings:important:"),
                )
            }
            "federation" => {
                let id = short_hash(&p.content);
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
            &self.redis,
            &index,
            &prefix,
            dims,
            cfg.redis_search.hnsw.m,
            cfg.redis_search.hnsw.ef_construction,
        )
        .await
        .with_context(|| format!("ensure index {index}"))?;

        // --- Write HASH + TTL ---
        let vector_bytes = bytemuck::cast_slice(&vector_f32).to_vec();
        let ts = Utc::now().timestamp();
        let ttl = p.ttl_seconds.unwrap_or(cfg.redis.default_ttl_seconds as u64);
        let tags_csv = if p.tags.is_empty() { "".into() } else { p.tags.join(",") };

        {
            let mut con = self.redis.get().await?;
            redis::pipe()
                .hset(&key, "content", &p.content)
                .hset(&key, "tags", tags_csv)
                .hset(&key, "importance", p.importance.unwrap_or_default())
                .hset(&key, "chain_id", p.chain_id.unwrap_or_default())
                .hset(&key, "thought_id", p.thought_id.unwrap_or_default())
                .hset(&key, "ts", ts)
                .hset(&key, "vector", vector_bytes)
                .expire(&key, ttl as usize)
                .query_async(&mut con)
                .await?;
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
}

// ---- helpers ----

fn help_text() -> String {
    r#"ui_context help:
- type="session-summaries": subagent/LLM session synthesis → {instance}:embeddings:session-summaries:{id}
- type="important":        long-running local context → {instance}:embeddings:important:{id}
- type="federation":       federation-wide context → Federation:embeddings:{id}
Required: content (except for help).
Optional: tags[], importance, chain_id, thought_id, ttl_seconds, instance_id."#.into()
}

async fn openai_embed(cfg: &Config, text: &str) -> Result<Vec<f32>> {
    let client = OAClient::new().with_api_key(cfg.openai.api_key()?);
    let req = CreateEmbeddingRequest {
        model: cfg.openai.embedding_model.clone().into(), // "text-embedding-3-small"
        input: EmbeddingInput::String(text.to_owned()),
        ..Default::default()
    };
    let resp = client.embeddings().create(req).await?;
    let first = resp.data.get(0).ok_or_else(|| anyhow!("no embedding returned"))?;
    Ok(first.embedding.clone())
}

fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let hex = hex::encode(h.finalize());
    hex[..16].to_string()
}

async fn ensure_index_if_needed(
    pool: &Pool,
    index: &str,
    prefix: &str,
    dims: usize,
    m: u32,
    ef_construction: u32,
) -> Result<bool> {
    let mut con = pool.get().await?;

    // Does FT.INFO exist?
    let info: redis::RedisResult<String> =
        redis::cmd("FT.INFO").arg(index).query_async(&mut con).await;

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
    redis::cmd("FT.CREATE")
        .arg(index)
        .arg("ON").arg("HASH")
        .arg("PREFIX").arg(1).arg(prefix)
        .arg("SCHEMA")
        .arg("content").arg("TEXT")
        .arg("tags").arg("TAG").arg("SEPARATOR").arg(",")
        .arg("importance").arg("TEXT")
        .arg("ts").arg("NUMERIC").arg("SORTABLE")
        .arg("vector").arg("VECTOR").arg("HNSW").arg(6)
            .arg("TYPE").arg("FLOAT32")
            .arg("DIM").arg(dims)
            .arg("DISTANCE_METRIC").arg("COSINE")
            .arg("M").arg(m)
            .arg("EF_CONSTRUCTION").arg(ef_construction)
        .query_async(&mut con)
        .await?;

    Ok(true)
}


⸻

4) Wire it into the build

If you don’t already have a mod tools; import, add it; then include the module:

// in src/lib.rs or wherever you collect tools
pub mod tools;

// in src/tools/mod.rs (create if missing)
pub mod ui_context;

Nothing else required if your ToolRouter is already the server’s router type.

⸻

5) Build it

cargo build --release

If this fails, read the error message like an adult and fix the path/import it complains about. The handler itself is fine on rmcp 0.5.x.

⸻

6) Minimal runtime test

Call the tool three ways and one help:
	•	session summary:

{
  "tool": "ui_context",
  "params": {
    "type": "session-summaries",
    "content": "Subagent synthesis: OODA loop started; importance=8; chain=20250808-Session1",
    "tags": ["DT","OODA","summary"],
    "importance": "high",
    "chain_id": "20250808-Session1"
  }
}

	•	important:

{
  "tool": "ui_context",
  "params": {
    "type": "important",
    "content": "Long-running constraint: prefer local inference for synthesis; avoid API spikes.",
    "tags": ["policy","cost"]
  }
}

	•	federation:

{
  "tool": "ui_context",
  "params": {
    "type": "federation",
    "content": "Federation guideline: session syntheses must be idempotent; hash-based dedupe.",
    "tags": ["federation","policy"]
  }
}

	•	help:

{
  "tool": "ui_context",
  "params": { "type": "help" }
}


⸻

7) Verify in Redis (sanity)
	•	Check hashes:
	•	redis-cli KEYS '*embeddings*'
	•	redis-cli HGETALL <key>
	•	Check indexes:
	•	redis-cli FT.INFO idx:CC:session-summaries
	•	redis-cli FT.INFO idx:CC:important
	•	redis-cli FT.INFO idx:Federation:embeddings

If FT.INFO errors with “unknown command,” RediSearch is not loaded. That’s not on this code.

⸻

Notes you’ll care about later
	•	The id is sha256(content)[:16]. If you want true updates, add a version field or store content_hash separate from the key and allow multiple versions per key.
	•	Search is trivial after this: same index names, KNN queries with your embedded query vector; you already have dims, cosine, and HNSW params aligned.
	•	Embedding model: OpenAI text-embedding-3-small (1536 dims). If you swap models, change embedding_dimensions in config and enjoy the cascade of consequences you created.