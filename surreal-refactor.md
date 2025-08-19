**SurrealDB Refactor Plan**

- **Scope:** Replace Redis (JSON, Hashes, Streams, Lua, RediSearch) with SurrealDB for storage, search, and graph. Limit initial surface to `ui_help`, `ui_think` (with embedding on write), `ui_remember` (unchanged interface; reads from embeddings), and `ui_knowledge` (graph CRUD + embedded search).
- **Non-Goals (initial):** Event streams, Bloom filters, rate metrics, legacy Redis search, non-listed tools.

**Why SurrealDB**

- **Unified model:** Documents, relations, and graph in one DB; reduces Lua/script surface.
- **Graph-native:** `RELATE` edges instead of manual relation indexes.
- **Embeddings-first:** Store vectors with records; enable vector + text search for thoughts and KG.

**Assumptions & Checks**

- **Version:** SurrealDB version supports full-text search and vector indexes. If vector index is unavailable, fall back to computing similarity server-side with a linear scan for small data or plug an external vector store behind a trait (see Risks).
- **Driver:** Use `surrealdb` crate with async Rust; connections via HTTP or WebSocket as per config.
- **Embeddings:** Use existing `generate_openai_embedding` helper; move cache to SurrealDB.

**Config Changes**

- **Add:** `surrealdb` block in `config.yaml` with `url`, `ns`, `db`, `user`, `pass`, `strict_ssl`, `vector_dim`, and `index_names`.
- **Remove/Defer:** Redis/Qdrant settings for this phase; keep for migrator.
- **ENV:** `SURREALDB_URL`, `SURREALDB_NS`, `SURREALDB_DB`, `SURREALDB_USER`, `SURREALDB_PASS`.

**Data Model**

- **Table `thoughts`**
  - Fields: `id` (record id or UUID), `instance` (string), `content` (string), `framework` (string?), `chain_id` (string?), `thought_number` (int), `total_thoughts` (int), `timestamp` (datetime), `importance` (int?), `relevance` (int?), `tags` (array<string>), `category` (string?), `embedding` (array<float>[dim]).
  - Indexes: by `instance`, by `chain_id`, full-text on `content`, vector on `embedding` (dim from config).

- **Table `chains`**
  - Fields: `id` (chain_id), `instance`, `created_at` (datetime), `thought_count` (int).
  - Indexes: unique by `id`, `instance+id`.

- **Table `kg_node`**
  - Fields: `id`, `name`, `display_name`, `entity_type` (enum/string), `scope` (enum/string), `created_at`, `updated_at`, `created_by`, `attributes` (object), `tags` (array<string>), `thought_ids` (array<string>), `embedding` (array<float>[dim]), `metadata` (object: auto_extracted, extraction_source, extraction_timestamp).
  - Indexes: unique on `(scope, lower(name))`, full-text on `name|display_name|tags|attributes`, vector on `embedding`.

- **Edge `kg_relation`**
  - Shape: `RELATE kg_node:$from -> kg_relation -> kg_node:$to SET relationship_type, scope, created_at, created_by, attributes, metadata{ bidirectional, weight }`.
  - Indexes: by `scope`, by `relationship_type`.

- **Table `session_state`**
  - Fields: `id` (session_key), `entity_id`, `scope`, `timestamp`.

- **Table `embedding_cache`** (optional)
  - Fields: `id` (hash(model+text)), `model`, `text_hash`, `embedding`.

**Schema DDL (SurrealQL, indicative)**

```sql
-- Namespaces and DB selected by connection settings

DEFINE TABLE thoughts SCHEMAFULL;
DEFINE FIELD instance       ON thoughts TYPE string;
DEFINE FIELD content        ON thoughts TYPE string;
DEFINE FIELD framework      ON thoughts TYPE string ASSERT $value != '' OR $value = NONE;
DEFINE FIELD chain_id       ON thoughts TYPE string ASSERT string::len($value) > 0 OR $value = NONE;
DEFINE FIELD thought_number ON thoughts TYPE int;
DEFINE FIELD total_thoughts ON thoughts TYPE int;
DEFINE FIELD timestamp      ON thoughts TYPE datetime;
DEFINE FIELD importance     ON thoughts TYPE int ASSERT $value >= 1 AND $value <= 10 OR $value = NONE;
DEFINE FIELD relevance      ON thoughts TYPE int ASSERT $value >= 1 AND $value <= 10 OR $value = NONE;
DEFINE FIELD tags           ON thoughts TYPE array<string> DEFAULT [];
DEFINE FIELD category       ON thoughts TYPE string OR NONE;
DEFINE FIELD embedding      ON thoughts TYPE array<float> ASSERT array::len($value) = $CONFIG.vector_dim;

DEFINE INDEX idx_thoughts_instance   ON thoughts FIELDS instance;
DEFINE INDEX idx_thoughts_chain      ON thoughts FIELDS chain_id;
DEFINE INDEX fts_thoughts_content    ON thoughts FIELDS content SEARCH ANALYZER english;
DEFINE INDEX v_idx_thoughts_embedding ON thoughts FIELDS embedding TYPE VECTOR DIM $CONFIG.vector_dim;

DEFINE TABLE chains SCHEMAFULL;
DEFINE FIELD id          ON chains TYPE string; -- chain_id
DEFINE FIELD instance    ON chains TYPE string;
DEFINE FIELD created_at  ON chains TYPE datetime;
DEFINE FIELD thought_count ON chains TYPE int;
DEFINE INDEX uniq_chain  ON chains COLUMNS id UNIQUE;

DEFINE TABLE kg_node SCHEMAFULL;
DEFINE FIELD name         ON kg_node TYPE string;
DEFINE FIELD display_name ON kg_node TYPE string;
DEFINE FIELD entity_type  ON kg_node TYPE string;
DEFINE FIELD scope        ON kg_node TYPE string; -- 'federation'|'personal'
DEFINE FIELD created_at   ON kg_node TYPE datetime;
DEFINE FIELD updated_at   ON kg_node TYPE datetime;
DEFINE FIELD created_by   ON kg_node TYPE string;
DEFINE FIELD attributes   ON kg_node TYPE object DEFAULT {};
DEFINE FIELD tags         ON kg_node TYPE array<string> DEFAULT [];
DEFINE FIELD thought_ids  ON kg_node TYPE array<string> DEFAULT [];
DEFINE FIELD embedding    ON kg_node TYPE array<float> ASSERT array::len($value) = $CONFIG.vector_dim OR $value = NONE;
DEFINE FIELD metadata     ON kg_node TYPE object DEFAULT { auto_extracted: false };
DEFINE INDEX uniq_node_name_scope ON kg_node COLUMNS string::lower(name), scope UNIQUE;
DEFINE INDEX fts_node ON kg_node FIELDS name, display_name, tags, attributes SEARCH ANALYZER english;
DEFINE INDEX v_idx_node_embedding ON kg_node FIELDS embedding TYPE VECTOR DIM $CONFIG.vector_dim;

DEFINE TABLE session_state SCHEMALESS; -- tiny helper table
DEFINE TABLE kg_relation SCHEMALESS;   -- edge type
```

Notes: Adjust ANALYZER and VECTOR index syntax to match installed SurrealDB release; if unsupported, see Risks.

**Repository Layer**

- **Keep traits:** Reuse `ThoughtRepository` and `KnowledgeRepository` unchanged.
- **New impls:** `SurrealThoughtRepository`, `SurrealKnowledgeRepository`, combined wrapper.
- **Connection manager:** `SurrealManager` with pooled connection, health check, and simple transaction helper.
- **Transactions:** Use Surreal `BEGIN … COMMIT` for multi-step writes (e.g., thought + chain upsert).

**Operation Mapping**

- **save_thought (ui_think):**
  - Validate input → generate embedding → `BEGIN`.
  - `CREATE thoughts SET …` with embedding vector.
  - If `chain_id`: `INSERT INTO chains` on conflict update `thought_count`, else `CREATE`.
  - `COMMIT`.
  - No Redis Bloom/Streams; emit application log instead.

- **chain_exists / save_chain_metadata:**
  - `SELECT id FROM chains WHERE id = $chain_id` → exists.
  - Upsert: `INSERT INTO chains … ON DUPLICATE KEY UPDATE thought_count = $n`.

- **get_thought / get_chain_thoughts / search_thoughts:**
  - By id: `SELECT * FROM thoughts WHERE id = $id AND instance = $instance`.
  - By chain: `SELECT * FROM thoughts WHERE instance = $instance AND chain_id = $chain_id ORDER BY timestamp`.
  - Search (hybrid):
    - Vector: `SELECT *, vector::cosine_distance(embedding, $q) AS score FROM thoughts WHERE instance = $instance ORDER BY score ASC LIMIT $k`.
    - Text: `SELECT * FROM thoughts WHERE instance = $instance AND content @1 $query LIMIT $k` (full-text syntax depends on version).
    - Merge and rerank in Rust by normalized score.

- **Knowledge CRUD (ui_knowledge):**
  - Entities: `CREATE/UPDATE/SELECT/DELETE` on `kg_node`.
  - Name index: rely on unique `(scope, lower(name))`; lookups via `SELECT * FROM kg_node WHERE scope=$scope AND string::lower(name)=string::lower($name)`.
  - Add thought to entity: `UPDATE kg_node SET thought_ids += [$thought_id] WHERE id = $id`.

- **Relations:**
  - Create: `RELATE $from -> kg_relation -> $to SET relationship_type=$t, scope=$s, created_at=time::now(), created_by=$who, attributes=$attrs, metadata=$meta`.
  - Get: `SELECT ->kg_relation->kg_node.* FROM (SELECT * FROM kg_node WHERE id=$id)` or `SELECT * FROM kg_relation WHERE in=$id OR out=$id` and hydrate in Rust.

- **ui_remember (read-only, unchanged interface):**
  - Query mode: accept query text; compute embedding; run hybrid search across `thoughts.embedding` and `kg_node.embedding`; unify results with types and scores; synthesize answer with existing `Synthesizer` using retrieved contexts only.
  - Feedback mode: same contract; persist any feedback payload into auxiliary tables later (deferred).

- **ui_help:**
  - Limit output to `ui_help`, `ui_think`, `ui_remember`, `ui_knowledge` with updated examples mentioning SurrealDB-backed embeddings.

**Server/Init Changes**

- **Service wiring:**
  - Add `SurrealManager` init in `main.rs`/`service.rs`; remove `RedisManager` from handler construction for this phase.
  - Feature flag `UI_STORAGE=surreal|redis` to allow side-by-side during migration.
  - Run one-time `DEFINE` DDL at startup if `APPLY_SCHEMA=true`.

- **Embeddings pipeline:**
  - Replace Redis caching with Surreal `embedding_cache` lookups via SHA256(text+model); write-through on miss.
  - Store vectors in-row for thoughts and optional for KG entities.

**Testing Plan**

- **Unit tests:** Keep repository traits mocked; extend tests for `ui_think` to assert embedding presence.
- **Integration (gated):** `#[ignore]` tests requiring live SurrealDB; use `SURREALDB_URL=memory` or docker in CI later.
- **No stdout noise:** Preserve current MCP transport hygiene.

**Migration Strategy**

- **Phase 0: Dual-write (optional short window):** Behind feature flag, write to both Redis and Surreal; read from Redis. Validate parity nightly.
- **Phase 1: Backfill:** Batch-export Redis JSON and relations; import into Surreal with small workers. Map keys:
  - `instance:Thoughts:{id}` → `thoughts`
  - `Chains:metadata:{chain}` → `chains`
  - KG JSON + Hash indices → `kg_node` and `RELATE` edges.
- **Phase 2: Read switch:** Toggle reads to Surreal; monitor. Keep Redis for rollback for a week.
- **Phase 3: Retire Redis:** Remove dual-write, deprecate Lua/Streams.

**Cutover & Verification**

- **Feature flag:** `UI_STORAGE=surreal` routes repository construction.
- **Smoke:** `ui_think` write + `ui_recall` by id/chain; `ui_knowledge` create/get/search; `ui_remember` returns contexts.
- **Acceptance:**
  - Thought writes include non-empty `embedding` of configured dimension.
  - KG create enforces unique name per scope; vector search returns expected neighbors.
  - `ui_help` lists only the four tools with Surreal wording.

**Risks & Mitigations**

- **Vector index availability:** If Surreal version lacks vector indexes, implement server-side hybrid search:
  - Maintain `LIMIT N` candidate set by text search, then compute cosine distance in Rust.
  - Or use table scan with paging for small datasets until index is enabled.
- **Query syntax drift:** Full-text `SEARCH` operators vary by release; gate behind small query builder.
- **Transactions semantics:** Validate atomicity for thought+chain upsert; if needed, collapse into single `CREATE` with deterministic IDs.
- **Throughput:** Start with sync writes; add background workers for heavy embedding generation if latency is high.

**Work Breakdown**

1) Plumbing & Config
- **Add:** `SurrealManager`, config parsing, env wiring, health check.
- **Flag:** `UI_STORAGE` to pick backend.

2) Schema & Bootstrapping
- **DDL:** Apply `DEFINE TABLE/INDEX` on startup (idempotent) or via a `scripts/surreal_schema.surql`.

3) Repositories
- **Implement:** `SurrealThoughtRepository` and `SurrealKnowledgeRepository` satisfying existing traits.
- **Hybrid search:** Add helper to combine vector/text results.

4) Handlers (tool-limited)
- **ui_think:** Call embeddings, write to `thoughts` (+ optional `chains`).
- **ui_knowledge:** CRUD + relations + optional embeddings on create/update.
- **ui_remember:** Use new search helpers for thoughts + KG.
- **ui_help:** Limit copy/examples to the 4 tools.

5) Tests & Docs
- **Update:** help text, README snippets; add unit tests for embedding presence and KG uniqueness.

6) Migration (parallel path)
- **Design:** Exporters/importers; not executed in initial PR.

**Deliverables**

- `src/surreal.rs` (manager)
- `src/repository_surreal.rs` with `SurrealThoughtRepository` and `SurrealKnowledgeRepository`
- DDL file `scripts/surreal_schema.surql`
- Config changes in `config.yaml`
- Handler updates scoped to four tools
- Minimal docs: this plan + help updates

**Next Steps (Decision Points)**

- Confirm SurrealDB deployment target and version for vector/FTS support.
- Choose embedding model and fix `vector_dim`.
- Approve feature-flagged rollout vs. hard switch.

