Future Fixes and Enhancements

- Retrieval optimization:
  - Use FT.SEARCH RETURN to include needed fields (`content`, `ts`) and reduce subsequent HGETALL round-trips when index schema allows.
  - Add optional per-index normalization for vector distances or switch to similarity scores when available, then tune the similarity mapping beyond 1/(1+distance).
- Hybrid scoring tunables:
  - Surface hybrid weights in config as profiles (e.g., YAML anchors) and allow runtime switching per request via params or chain metadata (opt-in).
  - Consider time-decay function customization (tau) in config.
- Context construction:
  - Replace the naive token estimator (chars/4) with a proper tokenizer to enforce context limits accurately.
- Feedback analytics:
  - Add background aggregation jobs to compute per-chain/session scores over time and surface metrics endpoints.
  - Expand heuristic to recognize “follow-up with correction” vs “clarification” more robustly.
- Index management:
  - Validate RediSearch index schemas at startup and emit actionable logs when misconfigured (e.g., vector field name, DIALECT compatibility).
  - Provide a small admin tool to ensure indices exist with correct schema.
- Caching:
  - Add optional cache for KNN results keyed by (query, index, top_k) with short TTL to reduce repeat queries in short windows.
- Security & CI:
  - Add a `deny.toml` with recommended policies and enable stricter gating when desired.
  - Add a lightweight secrets scan (e.g., gitleaks) in CI as non-blocking.
- Error handling:
  - Unify error categories for transport/embedding/redis errors and consider retry strategies for transient Redis operations.
- Qdrant integration:
  - Either reintroduce Qdrant-backed search as optional for other flows or remove dead code if unused to reduce maintenance.
- Testing:
  - Add integration tests behind a feature flag that spin up Redis via `testcontainers` when available.
  - Add property tests for the feedback scoring helper.

