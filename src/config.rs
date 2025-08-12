use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Main configuration structure for UnifiedIntelligence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub rate_limiter: RateLimiterConfig,
    pub event_stream: EventStreamConfig,
    pub bloom_filter: BloomFilterConfig,
    pub time_series: TimeSeriesConfig,
    pub retry: RetryConfig,
    pub qdrant: QdrantConfig,
    pub groq: GroqConfig,
    pub openai: OpenAIConfig,
    pub redis_search: RedisSearchConfig,
    pub ui_remember: UiRememberConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub version: String,
    pub default_instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub database: u8,
    pub pool: PoolConfig,
    pub default_ttl_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_size: usize,
    pub timeout_seconds: u64,
    pub create_timeout_seconds: u64,
    pub recycle_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    pub max_requests: u32,
    pub window_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamConfig {
    pub max_length: u64,
    pub approximate_trimming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloomFilterConfig {
    pub error_rate: f64,
    pub expected_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesConfig {
    pub retention_ms: u64,
    pub duplicate_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_base: f64,
    pub jitter_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    /// Similarity score threshold for filtering search results (0.0-1.0)
    pub similarity_threshold: f32,
    /// Host for Qdrant server
    pub host: String,
    /// Port for Qdrant server
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    pub api_key: String,
    pub intent_model: String,
    pub model_fast: String,
    pub model_deep: String,
}

impl Config {
    /// Load configuration from file with environment variable overrides
    /// ALWAYS returns a valid config - never fails
    pub fn load() -> Self {
        // Load environment variables from .env files
        // Try multiple locations since DT runs from different working directory
        let env_paths = [
            "/Users/samuelatagana/Projects/LegacyMind/.env", // Absolute path to centralized .env
            "../.env",                                       // Parent directory
            ".env",                                          // Current directory
        ];

        let mut env_loaded = false;
        for path in &env_paths {
            if dotenvy::from_path(path).is_ok() {
                tracing::info!("Loaded .env from: {}", path);
                env_loaded = true;
                break;
            }
        }

        if !env_loaded {
            tracing::warn!(
                "No .env file found in any expected location - continuing with env vars only"
            );
        }

        // Default config path
        let config_path = env::var("UI_CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());

        // Load config from file if it exists
        let mut config = if Path::new(&config_path).exists() {
            match fs::read_to_string(&config_path) {
                Ok(contents) => match serde_yaml::from_str::<Config>(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded configuration from {}", config_path);
                        config
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to parse config file {}: {} - using defaults",
                            config_path,
                            e
                        );
                        Self::default()
                    }
                },
                Err(e) => {
                    tracing::error!(
                        "Failed to read config file {}: {} - using defaults",
                        config_path,
                        e
                    );
                    Self::default()
                }
            }
        } else {
            tracing::warn!("Config file not found at {} - using defaults", config_path);
            Self::default()
        };

        // Apply environment variable overrides
        config.apply_env_overrides();
        // Apply preset mapping last to ensure it overrides weights if provided
        config.apply_ui_remember_preset();

        // Validate configuration - log warnings but don't fail
        if let Err(e) = config.validate() {
            tracing::warn!("Config validation warnings: {} - continuing anyway", e);
        }

        config
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // Server overrides
        if let Ok(name) = env::var("UI_SERVER_NAME") {
            self.server.name = name;
        }
        if let Ok(version) = env::var("UI_SERVER_VERSION") {
            self.server.version = version;
        }
        if let Ok(instance_id) = env::var("INSTANCE_ID") {
            self.server.default_instance_id = instance_id;
        }

        // Redis overrides
        if let Ok(host) = env::var("REDIS_HOST") {
            self.redis.host = host;
        }
        if let Ok(port) = env::var("REDIS_PORT") {
            if let Ok(port_num) = port.parse() {
                self.redis.port = port_num;
            }
        }
        if let Ok(db) = env::var("REDIS_DB") {
            if let Ok(db_num) = db.parse() {
                self.redis.database = db_num;
            }
        }

        // Pool overrides
        if let Ok(pool_size) = env::var("UI_REDIS_POOL_SIZE") {
            if let Ok(size) = pool_size.parse() {
                self.redis.pool.max_size = size;
            }
        }

        // Rate limiter overrides
        if let Ok(max_requests) = env::var("UI_RATE_LIMIT_MAX_REQUESTS") {
            if let Ok(max) = max_requests.parse() {
                self.rate_limiter.max_requests = max;
            }
        }
        if let Ok(window) = env::var("UI_RATE_LIMIT_WINDOW_SECONDS") {
            if let Ok(window_secs) = window.parse() {
                self.rate_limiter.window_seconds = window_secs;
            }
        }

        // Event stream overrides
        if let Ok(max_length) = env::var("UI_EVENT_STREAM_MAX_LENGTH") {
            if let Ok(max) = max_length.parse() {
                self.event_stream.max_length = max;
            }
        }

        // Retry overrides
        if let Ok(jitter) = env::var("UI_RETRY_JITTER_FACTOR") {
            if let Ok(jitter_val) = jitter.parse() {
                self.retry.jitter_factor = jitter_val;
            }
        }

        // Qdrant overrides
        if let Ok(threshold) = env::var("QDRANT_SIMILARITY_THRESHOLD") {
            if let Ok(threshold_val) = threshold.parse() {
                self.qdrant.similarity_threshold = threshold_val;
            }
        }
        if let Ok(host) = env::var("QDRANT_HOST") {
            self.qdrant.host = host;
        }
        if let Ok(port) = env::var("QDRANT_PORT") {
            if let Ok(port_num) = port.parse() {
                self.qdrant.port = port_num;
            }
        }

        // Groq overrides
        if let Ok(api_key) = env::var("GROQ_API_KEY") {
            self.groq.api_key = api_key;
        }
        if let Ok(intent_model) = env::var("GROQ_INTENT_MODEL") {
            self.groq.intent_model = intent_model;
        }
        if let Ok(model_fast) = env::var("GROQ_MODEL_FAST") {
            self.groq.model_fast = model_fast;
        }
        if let Ok(model_deep) = env::var("GROQ_MODEL_DEEP") {
            self.groq.model_deep = model_deep;
        }

        // ui_remember hybrid weight overrides
        if let Ok(w) = env::var("UI_REMEMBER_WEIGHT_SEMANTIC") {
            if let Ok(v) = w.parse() {
                self.ui_remember.hybrid_weights.semantic = v;
            }
        }
        if let Ok(w) = env::var("UI_REMEMBER_WEIGHT_TEXT") {
            if let Ok(v) = w.parse() {
                self.ui_remember.hybrid_weights.text = v;
            }
        }
        if let Ok(w) = env::var("UI_REMEMBER_WEIGHT_RECENCY") {
            if let Ok(v) = w.parse() {
                self.ui_remember.hybrid_weights.recency = v;
            }
        }
        if let Ok(preset) = env::var("UI_REMEMBER_PRESET") {
            self.ui_remember.preset = Some(preset);
        }
    }

    /// Validate configuration
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate Redis configuration
        if self.redis.port == 0 {
            return Err("Redis port cannot be 0".into());
        }

        // Validate rate limiter
        if self.rate_limiter.max_requests == 0 {
            return Err("Rate limiter max_requests cannot be 0".into());
        }
        if self.rate_limiter.window_seconds == 0 {
            return Err("Rate limiter window_seconds cannot be 0".into());
        }

        // Validate bloom filter
        if self.bloom_filter.error_rate <= 0.0 || self.bloom_filter.error_rate >= 1.0 {
            return Err("Bloom filter error rate must be between 0.0 and 1.0".into());
        }

        // Validate retry
        if self.retry.jitter_factor < 0.0 || self.retry.jitter_factor > 1.0 {
            return Err("Retry jitter factor must be between 0.0 and 1.0".into());
        }

        // Validate Qdrant configuration
        if self.qdrant.similarity_threshold < 0.0 || self.qdrant.similarity_threshold > 1.0 {
            return Err("Qdrant similarity threshold must be between 0.0 and 1.0".into());
        }
        if self.qdrant.port == 0 {
            return Err("Qdrant port cannot be 0".into());
        }

        // Validate Groq API key
        if self.groq.api_key == "PLACEHOLDER_GROQ_API_KEY" || self.groq.api_key.is_empty() {
            return Err("GROQ_API_KEY environment variable must be set".into());
        }

        // Validate ui_remember weights are sane (0..=1)
        let w = self.ui_remember.hybrid_weights;
        for (name, val) in [
            ("semantic", w.semantic),
            ("text", w.text),
            ("recency", w.recency),
        ] {
            if !(0.0..=1.0).contains(&val) {
                return Err(format!(
                    "ui_remember.hybrid_weights.{name} must be between 0.0 and 1.0"
                )
                .into());
            }
        }

        Ok(())
    }

    /// Get Redis URL with password from environment
    pub fn get_redis_url(&self) -> String {
        let password = env::var("REDIS_PASSWORD")
            .or_else(|_| env::var("REDIS_PASS"))
            .unwrap_or_else(|_| {
                tracing::warn!(
                    "REDIS_PASSWORD not set, assuming no password for local development."
                );
                "".to_string()
            });

        if password.is_empty() {
            format!(
                "redis://{}:{}/{}",
                self.redis.host, self.redis.port, self.redis.database
            )
        } else {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.redis.host, self.redis.port, self.redis.database
            )
        }
    }

    /// Get pool timeout as Duration
    pub fn get_pool_timeout(&self) -> Duration {
        Duration::from_secs(self.redis.pool.timeout_seconds)
    }

    /// Get pool create timeout as Duration
    pub fn get_pool_create_timeout(&self) -> Duration {
        Duration::from_secs(self.redis.pool.create_timeout_seconds)
    }

    /// Get pool recycle timeout as Duration
    pub fn get_pool_recycle_timeout(&self) -> Duration {
        Duration::from_secs(self.redis.pool.recycle_timeout_seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub embedding_model: String,
    pub embedding_dimensions: usize,
    pub api_key_env: Option<String>,
}

impl OpenAIConfig {
    #[allow(dead_code)]
    pub fn api_key(&self) -> anyhow::Result<String> {
        std::env::var("OPENAI_API_KEY").or_else(|_| {
            self.api_key_env
                .clone()
                .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY not set"))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisSearchConfig {
    pub hnsw: HNSWConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWConfig {
    pub m: u32,
    pub ef_construction: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                name: "unified-intelligence".to_string(),
                version: "3.0.0".to_string(),
                default_instance_id: "DT".to_string(),
            },
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                database: 0,
                pool: PoolConfig {
                    max_size: 16,
                    timeout_seconds: 5,
                    create_timeout_seconds: 5,
                    recycle_timeout_seconds: 5,
                },
                default_ttl_seconds: 604800,
            },
            rate_limiter: RateLimiterConfig {
                max_requests: 100,
                window_seconds: 60,
            },
            event_stream: EventStreamConfig {
                max_length: 10000,
                approximate_trimming: true,
            },
            bloom_filter: BloomFilterConfig {
                error_rate: 0.01,
                expected_items: 100000,
            },
            time_series: TimeSeriesConfig {
                retention_ms: 86400000,
                duplicate_policy: "SUM".to_string(),
            },
            retry: RetryConfig {
                max_attempts: 3,
                initial_delay_ms: 100,
                max_delay_ms: 5000,
                backoff_base: 2.0,
                jitter_factor: 0.1,
            },
            qdrant: QdrantConfig {
                similarity_threshold: 0.35,
                host: "localhost".to_string(),
                port: 6334,
            },
            groq: GroqConfig {
                api_key: env::var("GROQ_API_KEY").unwrap_or_else(|_| {
                    tracing::warn!("GROQ_API_KEY not set, using placeholder");
                    "PLACEHOLDER_GROQ_API_KEY".to_string()
                }),
                intent_model: "llama3-8b-8192".to_string(),
                model_fast: "llama3-8b-8192".to_string(),
                model_deep: "llama3-70b-8192".to_string(),
            },
            openai: OpenAIConfig {
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                api_key_env: None,
            },
            redis_search: RedisSearchConfig {
                hnsw: HNSWConfig {
                    m: 16,
                    ef_construction: 200,
                },
            },
            ui_remember: UiRememberConfig {
                hybrid_weights: HybridWeights {
                    semantic: 0.6,
                    text: 0.25,
                    recency: 0.15,
                },
                preset: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiRememberConfig {
    #[serde(default = "HybridWeights::default")]
    pub hybrid_weights: HybridWeights,
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub struct HybridWeights {
    pub semantic: f64,
    pub text: f64,
    pub recency: f64,
}

impl HybridWeights {
    fn default() -> Self {
        Self {
            semantic: 0.6,
            text: 0.25,
            recency: 0.15,
        }
    }
}

impl Config {
    fn apply_ui_remember_preset(&mut self) {
        if let Some(ref preset_raw) = self.ui_remember.preset {
            let preset = preset_raw.to_lowercase();
            let w = match preset.as_str() {
                // Prioritize speed and exact phrasing
                "fast-chat" => HybridWeights {
                    semantic: 0.45,
                    text: 0.40,
                    recency: 0.15,
                },
                // Prioritize semantic depth for synthesis
                "deep-research" => HybridWeights {
                    semantic: 0.75,
                    text: 0.10,
                    recency: 0.15,
                },
                // Emphasize most recent context
                "recall-recent" => HybridWeights {
                    semantic: 0.45,
                    text: 0.15,
                    recency: 0.40,
                },
                // Balanced default
                "balanced-default" => HybridWeights::default(),
                other => {
                    tracing::warn!(
                        "Unknown ui_remember preset: {}. Using existing weights.",
                        other
                    );
                    return;
                }
            };
            self.ui_remember.hybrid_weights = w;
            tracing::info!(
                "Applied ui_remember preset '{}': semantic={}, text={}, recency={}",
                preset,
                w.semantic,
                w.text,
                w.recency
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_remember_preset_mapping() {
        let mut cfg = Config::default();
        cfg.ui_remember.preset = Some("deep-research".to_string());
        cfg.apply_ui_remember_preset();
        let w = cfg.ui_remember.hybrid_weights;
        assert!((w.semantic - 0.75).abs() < 1e-9);
        assert!((w.text - 0.10).abs() < 1e-9);
        assert!((w.recency - 0.15).abs() < 1e-9);
    }

    #[test]
    fn test_ui_remember_preset_overrides_weights() {
        let mut cfg = Config::default();
        // Simulate prior weight overrides
        cfg.ui_remember.hybrid_weights.semantic = 0.55;
        cfg.ui_remember.hybrid_weights.text = 0.30;
        cfg.ui_remember.hybrid_weights.recency = 0.15;
        // Set preset and apply
        cfg.ui_remember.preset = Some("fast-chat".to_string());
        cfg.apply_ui_remember_preset();
        let w = cfg.ui_remember.hybrid_weights;
        assert!((w.semantic - 0.45).abs() < 1e-9);
        assert!((w.text - 0.40).abs() < 1e-9);
        assert!((w.recency - 0.15).abs() < 1e-9);
    }
}
