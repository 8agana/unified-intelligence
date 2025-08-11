use std::sync::Arc;

use deadpool::managed::QueueMode;
use deadpool_redis::{Config as DeadpoolConfig, Pool, PoolConfig, Runtime, Timeouts};
use redis::{AsyncCommands, JsonAsyncCommands, Script};

use sha2::{Digest, Sha256};

use crate::error::{Result, UnifiedIntelligenceError};
use crate::lua_scripts::{self, LoadedScripts};

/// Default TTL for all Redis writes (7 days in seconds)
const DEFAULT_TTL_SECONDS: i64 = 604800;

/// Redis connection manager
#[derive(Clone)]
pub struct RedisManager {
    pool: Arc<Pool>,
    scripts: Arc<tokio::sync::RwLock<LoadedScripts>>,
}

impl RedisManager {
    /// Create a new Redis manager with configuration
    pub async fn new_with_config(config: &crate::config::Config) -> Result<Self> {
        let redis_url = config.get_redis_url();

        tracing::info!(
            "Connecting to Redis at {}:{} (db: {})",
            config.redis.host,
            config.redis.port,
            config.redis.database
        );

        // Configure the connection pool with settings from config
        let mut cfg = DeadpoolConfig::from_url(&redis_url);

        // Set pool configuration from config
        cfg.pool = Some(PoolConfig {
            max_size: config.redis.pool.max_size,
            timeouts: Timeouts {
                wait: Some(config.get_pool_timeout()),
                create: Some(config.get_pool_create_timeout()),
                recycle: Some(config.get_pool_recycle_timeout()),
            },
            queue_mode: QueueMode::Fifo,
        });

        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| UnifiedIntelligenceError::PoolCreation(e.to_string()))?;

        // Test the connection
        let mut conn = pool.get().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Redis connection established");

        // Create instance with empty scripts for now
        let instance = Self {
            pool: Arc::new(pool),
            scripts: Arc::new(tokio::sync::RwLock::new(LoadedScripts::new())),
        };

        // Load Lua scripts
        instance.load_scripts().await?;

        Ok(instance)
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }

    /// Store a JSON object in Redis
    pub async fn json_set<T: serde::Serialize + Send + Sync>(
        &self,
        key: &str,
        path: &str,
        value: &T,
    ) -> Result<()> {
        let mut conn = self.get_connection().await?;
        conn.json_set::<_, _, _, ()>(key, path, value).await?;

        // Set TTL for the key (7 days)
        conn.expire::<_, ()>(key, DEFAULT_TTL_SECONDS).await?;
        Ok(())
    }

    /// Get a JSON object from Redis
    pub async fn json_get<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
        path: &str,
    ) -> Result<Option<T>> {
        let mut conn = self.get_connection().await?;

        // Use raw command to handle RedisJSON response
        let result: Option<String> = redis::cmd("JSON.GET")
            .arg(key)
            .arg(path)
            .query_async(&mut *conn)
            .await?;

        match result {
            Some(json_str) => {
                // When using "$" path, RedisJSON returns an array
                if path == "$" {
                    // Parse as array and get first element
                    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                        if let Some(first_value) = values.first() {
                            let value = serde_json::from_value(first_value.clone())?;
                            Ok(Some(value))
                        } else {
                            Ok(None)
                        }
                    } else {
                        // Try parsing directly if not an array
                        let value = serde_json::from_str(&json_str)?;
                        Ok(Some(value))
                    }
                } else {
                    // For other paths, parse directly
                    let value = serde_json::from_str(&json_str)?;
                    Ok(Some(value))
                }
            }
            None => Ok(None),
        }
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        Ok(conn.exists(key).await?)
    }

    // NOTE: The dangerous keys() method has been removed to prevent blocking operations.
    // Use scan_match() instead for pattern matching, which is non-blocking and production-safe.

    /// Add member to a set
    #[allow(dead_code)]
    pub async fn sadd(&self, key: &str, member: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        conn.sadd::<_, _, ()>(key, member).await?;

        // Set TTL for the key (7 days)
        conn.expire::<_, ()>(key, DEFAULT_TTL_SECONDS).await?;
        Ok(())
    }

    // ===== BOOST SCORE METHODS (Phase 3) =====

    /// Add entry to Redis Stream
    #[allow(dead_code)]
    pub async fn xadd(&self, key: &str, id: &str, fields: Vec<(&str, &str)>) -> Result<String> {
        let mut conn = self.get_connection().await?;

        // Build the XADD command
        let mut cmd = redis::cmd("XADD");
        cmd.arg(key).arg(id);

        // Add field-value pairs
        for (field, value) in fields {
            cmd.arg(field).arg(value);
        }

        let result: String = cmd.query_async(&mut *conn).await?;
        Ok(result)
    }

    // Timeout wrapper methods

    // Lua Script Methods

    /// Load all Lua scripts into Redis and store their SHA hashes
    pub async fn load_scripts(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let mut scripts = LoadedScripts::new();

        // Load store thought script
        let store_script = Script::new(lua_scripts::STORE_THOUGHT_SCRIPT);
        scripts.store_thought = store_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load store thought script: {e}"
                ))
            })?;

        // Load get thought script
        let get_script = Script::new(lua_scripts::GET_THOUGHT_SCRIPT);
        scripts.get_thought = get_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load get thought script: {e}"
                ))
            })?;

        // Load search thoughts script
        let search_script = Script::new(lua_scripts::SEARCH_THOUGHTS_SCRIPT);
        scripts.search_thoughts = search_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load search thoughts script: {e}"
                ))
            })?;

        // Load update chain script
        let update_chain_script = Script::new(lua_scripts::UPDATE_CHAIN_SCRIPT);
        scripts.update_chain = update_chain_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load update chain script: {e}"
                ))
            })?;

        // Load get chain thoughts script
        let get_chain_script = Script::new(lua_scripts::GET_CHAIN_THOUGHTS_SCRIPT);
        scripts.get_chain_thoughts = get_chain_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load get chain thoughts script: {e}"
                ))
            })?;

        // Load cleanup expired script
        let cleanup_script = Script::new(lua_scripts::CLEANUP_EXPIRED_SCRIPT);
        scripts.cleanup_expired = cleanup_script
            .prepare_invoke()
            .load_async(&mut *conn)
            .await
            .map_err(|e| {
                UnifiedIntelligenceError::Internal(format!(
                    "Failed to load cleanup expired script: {e}"
                ))
            })?;

        // Update the scripts in the instance
        let mut script_store = self.scripts.write().await;
        *script_store = scripts;

        tracing::info!("Successfully loaded all Lua scripts");

        Ok(())
    }

    /// Execute atomic thought storage using Lua script
    #[allow(clippy::too_many_arguments)]
    pub async fn store_thought_atomic(
        &self,
        thought_key: &str,
        bloom_key: &str,
        ts_key: &str,
        chain_key: Option<&str>,
        thought_json: &str,
        uuid: &str,
        timestamp: i64,
        chain_id: Option<&str>,
    ) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        // Prepare keys
        let mut keys = vec![thought_key, bloom_key, ts_key];
        if let Some(chain) = chain_key {
            keys.push(chain);
        } else {
            keys.push(""); // Placeholder
        }

        // Prepare arguments
        let args = vec![
            thought_json.to_string(),
            uuid.to_string(),
            timestamp.to_string(),
            chain_id.unwrap_or("").to_string(),
        ];

        // Get script SHA
        let script_sha = {
            let scripts = self.scripts.read().await;
            scripts.store_thought.clone()
        };

        // Execute script
        let result: String = redis::cmd("EVALSHA")
            .arg(&script_sha)
            .arg(keys.len())
            .arg(&keys)
            .arg(&args)
            .query_async(&mut *conn)
            .await
            .map_err(|e| {
                if e.to_string().contains("NOSCRIPT") {
                    tracing::warn!("Script not found in cache, reloading...");
                    UnifiedIntelligenceError::Internal("Script needs reloading".to_string())
                } else {
                    UnifiedIntelligenceError::Redis(e)
                }
            })?;

        match result.as_str() {
            "OK" => Ok(true),
            "DUPLICATE" => Ok(false),
            _ => Err(UnifiedIntelligenceError::Internal(format!(
                "Unexpected script result: {result}"
            ))),
        }
    }

    /// Get all thoughts in a chain using Lua script
    pub async fn get_chain_thoughts_atomic(
        &self,
        chain_key: &str,
        instance: &str,
    ) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;

        let keys = vec![chain_key];

        // Get script SHA
        let script_sha = {
            let scripts = self.scripts.read().await;
            scripts.get_chain_thoughts.clone()
        };

        let result: Vec<String> = redis::cmd("EVALSHA")
            .arg(&script_sha)
            .arg(keys.len())
            .arg(&keys)
            .arg(instance) // ARGV[1]
            .query_async(&mut *conn)
            .await
            .map_err(|e| {
                if e.to_string().contains("NOSCRIPT") {
                    tracing::warn!("Get chain thoughts script not found in cache");
                    UnifiedIntelligenceError::Internal("Script needs reloading".to_string())
                } else {
                    UnifiedIntelligenceError::Redis(e)
                }
            })?;

        Ok(result)
    }

    // Event Stream Methods

    /// Initialize event stream for an instance with max length
    pub async fn init_event_stream(&self, instance: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let stream_key = format!("{instance}:events");

        // Check if stream exists by trying to get info
        let exists: std::result::Result<Vec<Vec<String>>, _> = redis::cmd("XINFO")
            .arg("STREAM")
            .arg(&stream_key)
            .query_async(&mut *conn)
            .await;

        if exists.is_ok() {
            tracing::debug!("Event stream for instance {} already exists", instance);
            return Ok(());
        }

        // Create stream with initial entry
        let timestamp = "*"; // Let Redis assign timestamp
        let result: std::result::Result<String, _> = redis::cmd("XADD")
            .arg(&stream_key)
            .arg("MAXLEN")
            .arg("~") // Approximate trimming for performance
            .arg("10000") // Keep approximately 10k events
            .arg(timestamp)
            .arg("event_type")
            .arg("stream_initialized")
            .arg("instance")
            .arg(instance)
            .arg("timestamp")
            .arg(chrono::Utc::now().to_rfc3339())
            .query_async(&mut *conn)
            .await;

        match result {
            Ok(id) => {
                tracing::info!(
                    "Created event stream for instance {} with ID {}",
                    instance,
                    id
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to create event stream: {}", e);
                Err(UnifiedIntelligenceError::Redis(e))
            }
        }
    }

    /// Log a generic event to the stream
    pub async fn log_event(
        &self,
        instance: &str,
        event_type: &str,
        data: Vec<(&str, &str)>,
    ) -> Result<String> {
        let mut conn = self.get_connection().await?;
        let stream_key = format!("{instance}:events");

        // Build arguments for XADD
        let timestamp = chrono::Utc::now().to_rfc3339();
        let timestamp_ref = &timestamp;
        let mut args = vec![
            "MAXLEN",
            "~",
            "10000",
            "*", // Auto-generate ID
            "event_type",
            event_type,
            "instance",
            instance,
            "timestamp",
            timestamp_ref,
        ];

        // Add custom data fields
        for (key, value) in &data {
            args.push(key);
            args.push(value);
        }

        // Execute XADD
        let result: std::result::Result<String, _> = redis::cmd("XADD")
            .arg(&stream_key)
            .arg(&args)
            .query_async(&mut *conn)
            .await;

        match result {
            Ok(id) => {
                tracing::debug!(
                    "Logged {} event for instance {} with ID {}",
                    event_type,
                    instance,
                    id
                );
                Ok(id)
            }
            Err(e) => {
                tracing::error!("Failed to log event: {}", e);
                Err(UnifiedIntelligenceError::Redis(e))
            }
        }
    }

    /// Log a thought-specific event
    pub async fn log_thought_event(
        &self,
        instance: &str,
        event_type: &str,
        thought_id: &str,
        chain_id: Option<&str>,
        additional_data: Option<Vec<(&str, &str)>>,
    ) -> Result<String> {
        let mut data = vec![("thought_id", thought_id)];

        // Add chain_id if present
        if let Some(chain) = chain_id {
            data.push(("chain_id", chain));
        }

        // Add any additional data
        if let Some(extra) = additional_data {
            data.extend(extra);
        }

        self.log_event(instance, event_type, data).await
    }

    // Redis 8.0 Hash Field Expiration Methods

    /// Publish an event to Redis Streams for background processing
    pub async fn publish_stream_event(
        &self,
        instance: &str,
        event_type: &str,
        data: &serde_json::Value,
    ) -> Result<String> {
        let stream_key = format!("{instance}:events");
        let event_id = "*"; // Auto-generate timestamp

        let fields = vec![
            ("type", event_type.to_string()),
            ("data", data.to_string()),
            ("published_at", chrono::Utc::now().to_rfc3339()),
        ];

        let mut conn = self.get_connection().await?;
        let event_id: String = conn.xadd(&stream_key, event_id, &fields).await?;

        tracing::debug!(
            "Published {} event to stream {}: {}",
            event_type,
            stream_key,
            event_id
        );
        Ok(event_id)
    }

    /// Get a cached embedding from Redis
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn get_cached_embedding(&self, text: &str) -> Result<Option<Vec<f32>>> {
        let mut conn = self.get_connection().await?;
        let key = format!("embedding:{}", hex::encode(Sha256::digest(text))); // Use SHA256 hash of text as key

        let result: Option<Vec<u8>> = conn.get(&key).await?;

        match result {
            Some(bytes) => {
                let embedding: Vec<f32> = bincode::deserialize(&bytes).map_err(|e| {
                    UnifiedIntelligenceError::Internal(format!(
                        "Failed to deserialize embedding: {e}"
                    ))
                })?;
                Ok(Some(embedding))
            }
            None => Ok(None),
        }
    }

    /// Set a cached embedding in Redis with a TTL
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn set_cached_embedding(
        &self,
        text: &str,
        embedding: &[f32],
        ttl_seconds: i64,
    ) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let key = format!("embedding:{}", hex::encode(Sha256::digest(text))); // Use SHA256 hash of text as key

        let bytes = bincode::serialize(embedding).map_err(|e| {
            UnifiedIntelligenceError::Internal(format!("Failed to serialize embedding: {e}"))
        })?;

        conn.set_ex::<_, _, ()>(&key, bytes, ttl_seconds as u64)
            .await?;
        Ok(())
    }

    /// Execute RediSearch FT.SEARCH using Lua script
    pub async fn search_thoughts_rediseach(
        &self,
        index_name: &str,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;

        let keys = vec![index_name];
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let args = vec![query, &offset_str, &limit_str];

        let script_sha = {
            let scripts = self.scripts.read().await;
            scripts.search_thoughts.clone()
        };

        let result: Vec<String> = redis::cmd("EVALSHA")
            .arg(&script_sha)
            .arg(keys.len())
            .arg(&keys)
            .arg(&args)
            .query_async(&mut *conn)
            .await
            .map_err(|e| {
                if e.to_string().contains("NOSCRIPT") {
                    tracing::warn!("RediSearch script not found in cache, reloading...");
                    UnifiedIntelligenceError::Internal("Script needs reloading".to_string())
                } else {
                    UnifiedIntelligenceError::Redis(e)
                }
            })?;

        Ok(result)
    }
}
