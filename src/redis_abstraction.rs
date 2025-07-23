use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{warn, debug, error};
use async_trait::async_trait;

use crate::error::{Result, UnifiedIntelligenceError};
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::retry::{with_retry, RetryPolicy, RetryConfig};
use crate::redis::RedisManager;

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub is_healthy: bool,
    pub latency: Duration,
    pub error: Option<String>,
    pub timestamp: Instant,
}

/// Redis abstraction layer configuration
#[derive(Debug, Clone)]
pub struct RedisAbstractionConfig {
    pub circuit_breaker: CircuitBreakerConfig,
    pub retry_policy: RetryPolicy,
    pub health_check_interval: Duration,
    pub health_check_timeout: Duration,
}

impl Default for RedisAbstractionConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            retry_policy: RetryPolicy::ExponentialBackoff(RetryConfig {
                max_attempts: 3,
                base_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(10),
                jitter_factor: 0.1,
                backoff_multiplier: 2.0,
            }),
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
        }
    }
}

/// High-level Redis operations trait
#[async_trait]
pub trait RedisAbstraction: Send + Sync {
    // Core operations
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
    
    // JSON operations
    async fn json_set<T>(&self, key: &str, path: &str, value: &T) -> Result<()>
    where T: serde::Serialize + Send + Sync;
    async fn json_get<T>(&self, key: &str, path: &str) -> Result<Option<T>>
    where T: serde::de::DeserializeOwned + Send + Sync;
    async fn json_del(&self, key: &str, path: &str) -> Result<()>;
    
    // Batch operations
    async fn batch_get(&self, keys: &[String]) -> Result<Vec<Option<String>>>;
    async fn batch_json_get<T>(&self, keys_and_paths: &[(String, String)]) -> Result<Vec<Option<T>>>
    where T: serde::de::DeserializeOwned + Send + Sync;
    
    // Health and monitoring
    async fn health_check(&self) -> Result<HealthCheckResult>;
    async fn get_connection_info(&self) -> Result<RedisConnectionInfo>;
}

/// Connection information for monitoring
#[derive(Debug, Clone)]
pub struct RedisConnectionInfo {
    pub active_connections: usize,
    pub total_connections: usize,
    pub pending_requests: usize,
}

/// Enhanced Redis manager with circuit breaker and retry logic
pub struct EnhancedRedisManager {
    redis: Arc<RedisManager>,
    circuit_breaker: Arc<CircuitBreaker>,
    config: RedisAbstractionConfig,
    last_health_check: Arc<RwLock<Option<HealthCheckResult>>>,
}

impl EnhancedRedisManager {
    pub async fn new(config: RedisAbstractionConfig) -> Result<Self> {
        let redis = Arc::new(RedisManager::new().await?);
        let circuit_breaker = Arc::new(CircuitBreaker::new(config.circuit_breaker.clone()));
        
        let manager = Self {
            redis,
            circuit_breaker,
            config,
            last_health_check: Arc::new(RwLock::new(None)),
        };
        
        // Perform initial health check
        let _ = manager.health_check().await;
        
        Ok(manager)
    }
    
    /// Execute operation with circuit breaker and retry logic
    async fn execute_with_protection<F, T>(&self, operation_name: &str, operation: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send + 'static>> + Send + Sync,
    {
        // Check circuit breaker
        self.circuit_breaker.can_execute().await?;
        
        // Execute with retry logic
        let result = with_retry(
            || {
                let op = &operation;
                Box::pin(op())
            },
            self.config.retry_policy.clone(),
            operation_name,
        ).await;
        
        // Record result in circuit breaker
        match &result {
            Ok(_) => {
                self.circuit_breaker.record_success().await;
                debug!("Operation '{}' succeeded", operation_name);
            },
            Err(error) => {
                // Only record failures that aren't due to circuit breaker or max retries
                match error {
                    UnifiedIntelligenceError::CircuitBreakerOpen(_) |
                    UnifiedIntelligenceError::MaxRetriesExceeded { .. } => {
                        // Don't double-record these errors
                    },
                    _ => {
                        self.circuit_breaker.record_failure().await;
                        warn!("Operation '{}' failed: {}", operation_name, error);
                    }
                }
            }
        }
        
        result
    }
}

#[async_trait]
impl RedisAbstraction for EnhancedRedisManager {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let key = key.to_string();
        self.execute_with_protection("get", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            Box::pin(async move { redis.get(&key).await })
        }).await
    }
    
    async fn set(&self, key: &str, value: &str) -> Result<()> {
        let key = key.to_string();
        let value = value.to_string();
        self.execute_with_protection("set", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            let value = value.clone();
            Box::pin(async move { redis.set(&key, &value).await })
        }).await
    }
    
    async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()> {
        let key = key.to_string();
        let value = value.to_string();
        self.execute_with_protection("set_with_ttl", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            let value = value.clone();
            Box::pin(async move { redis.set_with_ttl(&key, &value, ttl_seconds).await })
        }).await
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        let key = key.to_string();
        self.execute_with_protection("delete", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            Box::pin(async move { redis.del(&key).await })
        }).await
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let key = key.to_string();
        self.execute_with_protection("exists", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            Box::pin(async move { redis.exists(&key).await })
        }).await
    }
    
    async fn json_set<T>(&self, key: &str, path: &str, value: &T) -> Result<()>
    where T: serde::Serialize + Send + Sync
    {
        let key = key.to_string();
        let path = path.to_string();
        let json_value = serde_json::to_value(value)?;
        
        self.execute_with_protection("json_set", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            let path = path.clone();
            let json_value = json_value.clone();
            Box::pin(async move { redis.json_set(&key, &path, &json_value).await })
        }).await
    }
    
    async fn json_get<T>(&self, key: &str, path: &str) -> Result<Option<T>>
    where T: serde::de::DeserializeOwned + Send + Sync
    {
        let key = key.to_string();
        let path = path.to_string();
        self.execute_with_protection("json_get", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            let path = path.clone();
            Box::pin(async move { redis.json_get::<T>(&key, &path).await })
        }).await
    }
    
    async fn json_del(&self, key: &str, path: &str) -> Result<()> {
        let key = key.to_string();
        let path = path.to_string();
        self.execute_with_protection("json_del", move || {
            let redis = self.redis.clone();
            let key = key.clone();
            let path = path.clone();
            Box::pin(async move { redis.json_del(&key, &path).await })
        }).await
    }
    
    async fn batch_get(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        let keys = keys.to_vec();
        self.execute_with_protection("batch_get", move || {
            let redis = self.redis.clone();
            let keys = keys.clone();
            Box::pin(async move { redis.batch_get(&keys).await })
        }).await
    }
    
    async fn batch_json_get<T>(&self, keys_and_paths: &[(String, String)]) -> Result<Vec<Option<T>>>
    where T: serde::de::DeserializeOwned + Send + Sync
    {
        let keys_and_paths = keys_and_paths.to_vec();
        self.execute_with_protection("batch_json_get", move || {
            let redis = self.redis.clone();
            let keys_and_paths = keys_and_paths.clone();
            Box::pin(async move { redis.batch_json_get(&keys_and_paths).await })
        }).await
    }
    
    async fn health_check(&self) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        
        let result = tokio::time::timeout(
            self.config.health_check_timeout,
            self.redis.ping()
        ).await;
        
        let latency = start_time.elapsed();
        
        let health_result = match result {
            Ok(Ok(_)) => HealthCheckResult {
                is_healthy: true,
                latency,
                error: None,
                timestamp: Instant::now(),
            },
            Ok(Err(e)) => HealthCheckResult {
                is_healthy: false,
                latency,
                error: Some(e.to_string()),
                timestamp: Instant::now(),
            },
            Err(_) => HealthCheckResult {
                is_healthy: false,
                latency,
                error: Some("Health check timeout".to_string()),
                timestamp: Instant::now(),
            },
        };
        
        // Update last health check
        *self.last_health_check.write().await = Some(health_result.clone());
        
        if health_result.is_healthy {
            debug!("Redis health check passed in {:?}", health_result.latency);
        } else {
            error!("Redis health check failed: {:?}", health_result.error);
        }
        
        Ok(health_result)
    }
    
    async fn get_connection_info(&self) -> Result<RedisConnectionInfo> {
        // This is a simplified implementation
        // In a real implementation, you'd query the pool for actual metrics
        Ok(RedisConnectionInfo {
            active_connections: 0, // Would need pool introspection
            total_connections: 16, // From pool configuration
            pending_requests: 0,   // Would need pool introspection
        })
    }
}