use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};
use rand::{thread_rng, Rng};

use crate::error::{Result, UnifiedIntelligenceError};

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay for exponential backoff
    pub base_delay: Duration,
    /// Maximum delay between attempts
    pub max_delay: Duration,
    /// Jitter factor to prevent thundering herd (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.1,
            backoff_multiplier: 2.0,
        }
    }
}

/// Retry policy for different operation types
#[derive(Debug, Clone)]
pub enum RetryPolicy {
    /// No retries
    None,
    /// Fixed delay between attempts
    Fixed(Duration),
    /// Exponential backoff with configuration
    ExponentialBackoff(RetryConfig),
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::ExponentialBackoff(RetryConfig::default())
    }
}

/// Determine if an error is retryable
pub fn is_retryable_error(error: &UnifiedIntelligenceError) -> bool {
    match error {
        // Connection-related errors are retryable
        UnifiedIntelligenceError::Redis(_) => true,
        UnifiedIntelligenceError::Pool(_) => true,
        UnifiedIntelligenceError::PoolGet(_) => true,
        UnifiedIntelligenceError::Timeout(_) => true,
        UnifiedIntelligenceError::ConnectionUnavailable(_) => true,
        UnifiedIntelligenceError::HealthCheckFailed(_) => true,
        
        // Circuit breaker and business logic errors are not retryable
        UnifiedIntelligenceError::CircuitBreakerOpen(_) => false,
        UnifiedIntelligenceError::Validation { .. } => false,
        UnifiedIntelligenceError::NotFound(_) => false,
        UnifiedIntelligenceError::RateLimit => false,
        UnifiedIntelligenceError::Unauthorized => false,
        UnifiedIntelligenceError::MaxRetriesExceeded { .. } => false,
        
        // Configuration and serialization errors are not retryable
        UnifiedIntelligenceError::Configuration(_) => false,
        UnifiedIntelligenceError::Serialization(_) => false,
        UnifiedIntelligenceError::Json(_) => false,
        
        // Other errors might be retryable depending on context
        UnifiedIntelligenceError::Internal(_) => true,
        UnifiedIntelligenceError::SearchUnavailable(_) => true,
        UnifiedIntelligenceError::Python(_) => false,
        UnifiedIntelligenceError::PoolCreation(_) => false,
        UnifiedIntelligenceError::InvalidAction(_) => false,
        UnifiedIntelligenceError::ChainOperation(_) => true,
        UnifiedIntelligenceError::Cancelled(_) => false,
        UnifiedIntelligenceError::DuplicateThought { .. } => false,
    }
}

/// Execute a function with retry logic
pub async fn with_retry<F, Fut, T>(
    operation: F,
    policy: RetryPolicy,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    match policy {
        RetryPolicy::None => operation().await,
        RetryPolicy::Fixed(delay) => {
            execute_with_fixed_retry(operation, delay, 3, operation_name).await
        },
        RetryPolicy::ExponentialBackoff(config) => {
            execute_with_exponential_backoff(operation, config, operation_name).await
        },
    }
}

async fn execute_with_fixed_retry<F, Fut, T>(
    operation: F,
    delay: Duration,
    max_attempts: u32,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;
    
    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(
                        "Operation '{}' succeeded on attempt {}/{}",
                        operation_name, attempt, max_attempts
                    );
                }
                return Ok(result);
            },
            Err(error) => {
                if !is_retryable_error(&error) {
                    debug!(
                        "Operation '{}' failed with non-retryable error: {}",
                        operation_name, error
                    );
                    return Err(error);
                }
                
                last_error = Some(error);
                
                if attempt < max_attempts {
                    warn!(
                        "Operation '{}' failed on attempt {}/{}, retrying in {:?}: {}",
                        operation_name, attempt, max_attempts, delay, last_error.as_ref().unwrap()
                    );
                    sleep(delay).await;
                } else {
                    warn!(
                        "Operation '{}' failed after {} attempts: {}",
                        operation_name, max_attempts, last_error.as_ref().unwrap()
                    );
                }
            }
        }
    }
    
    Err(UnifiedIntelligenceError::MaxRetriesExceeded { 
        attempts: max_attempts 
    })
}

async fn execute_with_exponential_backoff<F, Fut, T>(
    operation: F,
    config: RetryConfig,
    operation_name: &str,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;
    let mut delay = config.base_delay;
    
    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(
                        "Operation '{}' succeeded on attempt {}/{}",
                        operation_name, attempt, config.max_attempts
                    );
                }
                return Ok(result);
            },
            Err(error) => {
                if !is_retryable_error(&error) {
                    debug!(
                        "Operation '{}' failed with non-retryable error: {}",
                        operation_name, error
                    );
                    return Err(error);
                }
                
                last_error = Some(error);
                
                if attempt < config.max_attempts {
                    // Calculate delay with jitter
                    let jittered_delay = apply_jitter(delay, config.jitter_factor);
                    
                    warn!(
                        "Operation '{}' failed on attempt {}/{}, retrying in {:?}: {}",
                        operation_name, attempt, config.max_attempts, jittered_delay, 
                        last_error.as_ref().unwrap()
                    );
                    
                    sleep(jittered_delay).await;
                    
                    // Calculate next delay with exponential backoff
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * config.backoff_multiplier) as u64
                        ),
                        config.max_delay
                    );
                } else {
                    warn!(
                        "Operation '{}' failed after {} attempts: {}",
                        operation_name, config.max_attempts, last_error.as_ref().unwrap()
                    );
                }
            }
        }
    }
    
    Err(UnifiedIntelligenceError::MaxRetriesExceeded { 
        attempts: config.max_attempts 
    })
}

/// Apply jitter to delay to prevent thundering herd
fn apply_jitter(delay: Duration, jitter_factor: f64) -> Duration {
    if jitter_factor <= 0.0 {
        return delay;
    }
    
    let mut rng = thread_rng();
    let jitter_amount = delay.as_millis() as f64 * jitter_factor;
    let jitter = rng.gen_range(-jitter_amount..=jitter_amount);
    let new_millis = (delay.as_millis() as f64 + jitter).max(0.0) as u64;
    
    Duration::from_millis(new_millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;
    
    #[test]
    async fn test_apply_jitter() {
        let delay = Duration::from_millis(1000);
        let jittered = apply_jitter(delay, 0.1);
        
        // Should be within 10% of original delay
        assert!(jittered.as_millis() >= 900);
        assert!(jittered.as_millis() <= 1100);
    }
    
    #[test]
    async fn test_is_retryable_error() {
        assert!(is_retryable_error(&UnifiedIntelligenceError::Internal("test".to_string())));
        assert!(!is_retryable_error(&UnifiedIntelligenceError::Validation {
            field: "test".to_string(),
            reason: "test".to_string()
        }));
    }
}