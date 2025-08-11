# Resilient Startup Architecture for UnifiedIntelligence MCP

## Goal
The MCP server should ALWAYS start, regardless of missing or misconfigured dependencies. It should:
1. Start successfully even if all external services are down
2. Log what's available and what's not
3. Operate in degraded mode when services are unavailable
4. Attempt to reconnect to services as they become available

## Current Problems
1. **Config Loading**: Fails if config file is missing or malformed
2. **Redis Connection**: Fails if Redis is unreachable
3. **Qdrant Connection**: Fails if Qdrant is unreachable
4. **API Keys**: Missing API keys cause failures in various tools

## Proposed Architecture

### 1. Service Availability Status
```rust
#[derive(Debug, Clone)]
pub enum ServiceStatus {
    Available,
    Unavailable(String), // reason
    Degraded(String),    // partial functionality
}

pub struct ServiceHealth {
    redis: ServiceStatus,
    qdrant: ServiceStatus,
    openai: ServiceStatus,
    groq: ServiceStatus,
    last_check: chrono::DateTime<chrono::Utc>,
}
```

### 2. Lazy Initialization Pattern
Instead of connecting at startup, services connect on first use:

```rust
pub struct LazyRedisManager {
    config: Arc<Config>,
    pool: Arc<RwLock<Option<Pool>>>,
    status: Arc<RwLock<ServiceStatus>>,
}

impl LazyRedisManager {
    pub async fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            pool: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(ServiceStatus::Unavailable("Not initialized".into()))),
        }
    }
    
    pub async fn get_connection(&self) -> Result<Option<Connection>> {
        // Try to get existing connection
        if let Some(pool) = self.pool.read().await.as_ref() {
            match pool.get().await {
                Ok(conn) => return Ok(Some(conn)),
                Err(e) => {
                    log::warn!("Redis connection failed: {}", e);
                }
            }
        }
        
        // Try to initialize if not already done
        self.try_initialize().await;
        
        // Return None if still unavailable
        if let Some(pool) = self.pool.read().await.as_ref() {
            pool.get().await.ok()
        } else {
            None
        }
    }
    
    async fn try_initialize(&self) -> bool {
        // Implementation...
    }
}
```

### 3. Tool-Level Degradation
Each tool checks service availability:

```rust
impl UnifiedIntelligenceService {
    pub async fn ui_think(&self, params: UiThinkParams) -> Result<ThinkResponse> {
        // Core thinking always works (no external deps)
        let thought = process_thought(params);
        
        // Try to store in Redis
        match self.redis.get_connection().await {
            Some(conn) => {
                // Store thought
                log::debug!("Stored thought in Redis");
            }
            None => {
                log::warn!("Redis unavailable, thought not persisted");
                // Could fall back to local file
                self.fallback_storage.store(&thought).await;
            }
        }
        
        Ok(thought)
    }
    
    pub async fn ui_remember(&self, params: UiRememberParams) -> Result<RememberResponse> {
        // Check dependencies
        if !self.is_groq_available() {
            return Ok(RememberResponse {
                status: "degraded",
                message: "Groq API unavailable - synthesis disabled",
                ..Default::default()
            });
        }
        
        // Continue with available services...
    }
}
```

### 4. Resilient Config Loading
```rust
impl Config {
    pub fn load_resilient() -> Self {
        // Try to load from file
        match Self::load_from_file() {
            Ok(config) => {
                log::info!("Loaded config from file");
                config
            }
            Err(e) => {
                log::warn!("Config file error: {}, using defaults", e);
                let mut config = Self::default();
                
                // Apply any available env vars
                config.apply_env_overrides();
                
                config
            }
        }
    }
}
```

### 5. Health Check Endpoint
Add a tool that reports system health:

```rust
pub async fn ui_health(&self) -> HealthReport {
    HealthReport {
        status: "operational", // always operational, even if degraded
        services: {
            redis: self.redis.status(),
            qdrant: self.qdrant.status(),
            openai: self.openai.status(),
            groq: self.groq.status(),
        },
        degraded_tools: vec![
            // List tools with reduced functionality
        ],
        message: "System operational with limitations",
    }
}
```

## Implementation Phases

### Phase 1: Resilient Config (Immediate)
- Make all config fields have defaults
- Config file becomes optional
- Log missing config clearly

### Phase 2: Lazy Redis (Next)
- Redis connection becomes lazy
- Tools gracefully handle missing Redis
- Add local file fallback for thoughts

### Phase 3: Service Health Monitoring
- Add health check tool
- Periodic reconnection attempts
- Status reporting

### Phase 4: Full Fallback Chain
- LanceDB as secondary storage
- Obsidian as tertiary storage
- Memory-only as last resort

## Testing Strategy
1. Start with no config file → should use defaults
2. Start with Redis down → should start and log warning
3. Start with wrong Redis password → should start and retry
4. Start with no API keys → should start with some tools degraded
5. Start with EVERYTHING wrong → should still start!

## Success Criteria
- [ ] MCP starts even if all external services are down
- [ ] Clear logging of what's available and what's not
- [ ] Tools report degraded functionality clearly
- [ ] No silent failures - everything is logged
- [ ] Reconnection attempts for down services
- [ ] Health check tool shows current status
