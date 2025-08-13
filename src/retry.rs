/// Retry policy configuration
#[derive(Debug, Clone, Default)]
pub struct RetryConfig {}

/// Retry policy for different operation types
#[derive(Debug, Clone)]
pub enum RetryPolicy {
    /// Exponential backoff with configuration
    ExponentialBackoff(RetryConfig),
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::ExponentialBackoff(RetryConfig::default())
    }
}

#[cfg(test)]
mod tests {}
