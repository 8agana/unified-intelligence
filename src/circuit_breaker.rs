/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerState {}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {}
    }
}
