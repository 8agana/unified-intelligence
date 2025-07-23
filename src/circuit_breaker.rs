use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{Result, UnifiedIntelligenceError};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerState {
    Closed,  // Normal operation
    Open,    // Failures exceeded threshold, blocking requests
    HalfOpen, // Testing if service has recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Timeout before attempting recovery
    pub timeout: Duration,
    /// Success threshold in half-open state before closing
    pub success_threshold: u32,
    /// Window size for tracking recent failures
    pub window_size: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            window_size: 10,
        }
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    recent_results: Arc<RwLock<Vec<bool>>>, // true = success, false = failure
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            recent_results: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Check if request should be allowed through
    pub async fn can_execute(&self) -> Result<()> {
        let state = self.state.read().await;
        
        match *state {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::Open => {
                // Check if timeout period has passed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.timeout {
                        drop(state); // Release read lock
                        self.transition_to_half_open().await;
                        Ok(())
                    } else {
                        Err(UnifiedIntelligenceError::CircuitBreakerOpen(
                            "Circuit breaker is open, blocking request".to_string()
                        ))
                    }
                } else {
                    // No last failure time, should not be in open state
                    drop(state);
                    self.transition_to_closed().await;
                    Ok(())
                }
            },
            CircuitBreakerState::HalfOpen => Ok(()),
        }
    }

    /// Record successful operation
    pub async fn record_success(&self) {
        let mut recent_results = self.recent_results.write().await;
        recent_results.push(true);
        if recent_results.len() > self.config.window_size {
            recent_results.remove(0);
        }

        let state = *self.state.read().await;
        match state {
            CircuitBreakerState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;
                
                if *success_count >= self.config.success_threshold {
                    drop(success_count); // Release write lock
                    self.transition_to_closed().await;
                    info!("Circuit breaker closed after successful recovery");
                }
            },
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
            },
            _ => {}
        }
    }

    /// Record failed operation
    pub async fn record_failure(&self) {
        let mut recent_results = self.recent_results.write().await;
        recent_results.push(false);
        if recent_results.len() > self.config.window_size {
            recent_results.remove(0);
        }
        drop(recent_results);

        let state = *self.state.read().await;
        match state {
            CircuitBreakerState::Closed => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;
                
                if *failure_count >= self.config.failure_threshold {
                    drop(failure_count); // Release write lock
                    self.transition_to_open().await;
                    warn!("Circuit breaker opened due to {} failures", self.config.failure_threshold);
                }
            },
            CircuitBreakerState::HalfOpen => {
                self.transition_to_open().await;
                warn!("Circuit breaker reopened due to failure during recovery");
            },
            _ => {}
        }
    }

    /// Get current state
    pub async fn state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    /// Get metrics for monitoring
    pub async fn metrics(&self) -> CircuitBreakerMetrics {
        let recent_results = self.recent_results.read().await;
        let successes = recent_results.iter().filter(|&&r| r).count();
        let _failures = recent_results.iter().filter(|&&r| !r).count();
        
        CircuitBreakerMetrics {
            state: self.state().await,
            failure_count: *self.failure_count.read().await,
            success_count: *self.success_count.read().await,
            recent_success_rate: if recent_results.is_empty() {
                0.0
            } else {
                successes as f32 / recent_results.len() as f32
            },
            recent_operations: recent_results.len(),
        }
    }

    async fn transition_to_closed(&self) {
        *self.state.write().await = CircuitBreakerState::Closed;
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
    }

    async fn transition_to_open(&self) {
        *self.state.write().await = CircuitBreakerState::Open;
        *self.last_failure_time.write().await = Some(Instant::now());
        *self.success_count.write().await = 0;
    }

    async fn transition_to_half_open(&self) {
        *self.state.write().await = CircuitBreakerState::HalfOpen;
        *self.success_count.write().await = 0;
    }
}

/// Metrics for circuit breaker monitoring
#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub recent_success_rate: f32,
    pub recent_operations: usize,
}