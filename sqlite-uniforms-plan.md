**SQLite Uniforms Plan**

- **Scope:** Implement an embedded, uniform-scoped cognitive substrate using SQLite for: `ui_help` (limited), `ui_think` (embed on write), `ui_remember` (hybrid retrieval), and `ui_knowledge` (KG CRUD + frames + evidence). No Docker, no external daemons.
- **Non-goals (initial):** Redis Streams/eventing, distributed concurrency, cross-uniform joins beyond core attachment, non-listed tools.

**Concepts**

- **Uniforms:** Context-specific databases (e.g., `photography.db`, `legacymind.db`) to isolate knowledge per role/mode. Keeps retrieval relevant and fast by reducing corpus size.
- **Core DB:** `core.db` holds universal identity/preferences. Uniform reads may include core read-only.
- **Frames (Hawkins-inspired):** Per-node perspectives: `semantic`, `episodic`, `procedural`, `custom`, each with its own properties, evidence, and optional embedding.
- **Evidence:** Every fact/relation/claim can reference source `thought_id`s for explainability and contradiction checks.
- **Score Fusion:** Retrieval combines semantic similarity, recency, relation weight, task/intent match, and penalties for conflicts.

**Directory Layout**

- `Memory/uniforms/`
  - `core.db`
  - `<uniform>.db` (e.g., `photography.db`, `legacymind.db`, `sysadmin.db`)
  - `indexes/` (optional for persisted HNSW indexes if not using sqlite-vec)

**Config**

- `UI_UNIFORM`: active uniform name (e.g., `photography`).
- `UNIFORMS_DIR`: base path for DBs (default `Memory/uniforms`).
- `INCLUDE_CORE`: `true|false` (default `true`) — attach `core.db` read-only for queries.
- `VECTOR_DIM`: embedding vector size (e.g., 1536).
- `VECTORS_BACKEND`: `sqlite-vec|hnsw` — choose vector search implementation.

**Uniform Router (runtime)**

- Opens `UNIFORMS_DIR/<UI_UNIFORM>.db` with `WAL`, `busy_timeout=5s`.
- If `INCLUDE_CORE`, `ATTACH DATABASE '<dir>/core.db' AS core` read-only.
- Provides `conn()` for repository calls; exposes helpers to run uniform-only or core+uniform queries.
- Migration on open: apply schema if `schema_version` differs.

**Schema (per-uniform DB)**

- Pragmas on open: `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;`

- `thoughts`
  - `id TEXT PRIMARY KEY` (UUID)
  - `content TEXT NOT NULL`
  - `framework TEXT` (conversation|debug|build|stuck|review)
  - `chain_id TEXT` NULL
  - `thought_number INTEGER NOT NULL`
  - `total_thoughts INTEGER NOT NULL`
  - `timestamp TEXT NOT NULL` (RFC3339)
  - `importance INTEGER NULL`
  - `relevance INTEGER NULL`
  - `tags TEXT` (JSON array)
  - `category TEXT NULL`
  - `embedding BLOB` (vector; see Vectors)
  - Signals: `novelty REAL`, `consistency REAL`, `utility REAL`

- `chains`
  - `id TEXT PRIMARY KEY`
  - `created_at TEXT NOT NULL`
  - `thought_count INTEGER NOT NULL`

- `kg_node`
  - `id TEXT PRIMARY KEY`
  - `name TEXT NOT NULL`
  - `display_name TEXT NOT NULL`
  - `entity_type TEXT NOT NULL` (issue|person|system|concept|tool|framework|custom)
  - `scope TEXT` (personal|federation|NULL)
  - `created_at TEXT NOT NULL`
  - `updated_at TEXT NOT NULL`
  - `created_by TEXT NOT NULL`
  - `attributes TEXT NOT NULL` (JSON)
  - `tags TEXT NOT NULL` (JSON array)
  - `embedding BLOB` NULL
  - `auto_extracted INTEGER NOT NULL DEFAULT 0` (bool)

- `kg_frame`
  - `id TEXT PRIMARY KEY`
  - `node_id TEXT NOT NULL REFERENCES kg_node(id) ON DELETE CASCADE`
  - `column TEXT NOT NULL` (semantic|episodic|procedural|custom)
  - `properties TEXT NOT NULL` (JSON)
  - `embedding BLOB` NULL
  - `created_at TEXT NOT NULL`
  - `updated_at TEXT NOT NULL`

- `frame_evidence`
  - `frame_id TEXT NOT NULL REFERENCES kg_frame(id) ON DELETE CASCADE`
  - `thought_id TEXT NOT NULL REFERENCES thoughts(id) ON DELETE CASCADE`
  - `PRIMARY KEY(frame_id, thought_id)`

- `kg_relation`
  - `id TEXT PRIMARY KEY`
  - `from_id TEXT NOT NULL REFERENCES kg_node(id) ON DELETE CASCADE`
  - `to_id TEXT NOT NULL REFERENCES kg_node(id) ON DELETE CASCADE`
  - `relationship_type TEXT NOT NULL`
  - `bidirectional INTEGER NOT NULL DEFAULT 0`
  - `weight REAL NOT NULL DEFAULT 1.0`
  - `confidence REAL NULL`
  - `temporal_start TEXT NULL`
  - `temporal_end TEXT NULL`
  - `created_at TEXT NOT NULL`
  - `created_by TEXT NOT NULL`
  - `attributes TEXT NOT NULL` (JSON)

- `relation_evidence`
  - `relation_id TEXT NOT NULL REFERENCES kg_relation(id) ON DELETE CASCADE`
  - `thought_id TEXT NOT NULL REFERENCES thoughts(id) ON DELETE CASCADE`
  - `PRIMARY KEY(relation_id, thought_id)`

- `working_memory` (optional)
  - `key TEXT PRIMARY KEY`
  - `value TEXT NOT NULL` (JSON)
  - `expires_at TEXT NULL`

- `embedding_cache` (optional write-through cache)
  - `hash TEXT PRIMARY KEY` (sha256(model+text))
  - `model TEXT NOT NULL`
  - `dim INTEGER NOT NULL`
  - `vector BLOB NOT NULL`
  - `created_at TEXT NOT NULL`

- Indexes
  - `CREATE INDEX idx_thoughts_chain ON thoughts(chain_id);`
  - `CREATE INDEX idx_thoughts_ts ON thoughts(timestamp);`
  - `CREATE INDEX idx_node_name ON kg_node(name);`
  - `CREATE INDEX idx_frame_node ON kg_frame(node_id);`
  - `CREATE INDEX idx_rel_from_to ON kg_relation(from_id, to_id);`
  - `CREATE INDEX idx_rel_type ON kg_relation(relationship_type);`

- FTS5 (lexical)
  - `CREATE VIRTUAL TABLE thoughts_fts USING fts5(content, content='thoughts', content_rowid='rowid');`
  - Triggers to sync `thoughts` -> `thoughts_fts` on INSERT/UPDATE/DELETE.
  - Optional `kg_nodes_fts(name, display_name, tags, attributes_text)` for semantic labels.

**Vectors**

- Mode A: `sqlite-vec` (preferred if available)
  - Store embeddings in `thoughts.embedding` and `kg_node.embedding`.
  - Create vector indexes via sqlite-vec; query top-k with filters (e.g., `WHERE` on tags/category via outer query).
- Mode B: in-process HNSW (`hnsw_rs`)
  - Maintain an HNSW index per table (thoughts, kg_node), persisted under `Memory/uniforms/indexes/<uniform>_thoughts.hnsw`.
  - Rebuild on startup from SQLite if index missing or mismatched `VECTOR_DIM`.
  - Filter-aware search: pre-filter candidates in SQLite, then kNN on narrowed set.

**Core Attachment**

- On query, if `INCLUDE_CORE=true`:
  - `ATTACH DATABASE '<dir>/core.db' AS core;`
  - Union uniform+core results for FTS and vector candidates; merge/rerank in Rust.
  - Writes remain uniform-only (core is read-only at runtime).

**Handler Mapping**

- `ui_think`
  - Validate, generate embedding.
  - INSERT `thoughts` row with embedding and computed signals:
    - `novelty = 1 - max_cosine(last_N_thoughts)`
    - `consistency` (optional first pass: 1.0)
    - `utility` (from feedback moving average, initially NULL)
  - Optional: auto-extract simple entities/relations to create a `kg_frame(column='semantic')` with `frame_evidence` referencing the new thought.

- `ui_knowledge`
  - CRUD `kg_node` and `kg_relation` (with `weight`, `bidirectional`, `confidence`).
  - Frames: `kg_frame` create/update with properties + evidence.
  - Optional embeddings for nodes/frames via `generate_openai_embedding`.

- `ui_remember`
  - Build query embedding; retrieve candidates:
    - Thought kNN (uniform [+ core]) with filters.
    - Node kNN (uniform [+ core]).
    - FTS matches from `thoughts_fts` and `kg_nodes_fts`.
  - Merge candidates, dedupe by entity; expand 1 hop over `kg_relation` using high-weight edges.
  - Score fusion: `final = w_sem*semantic + w_epi*(recency_decay)*episodic + w_proc*procedure_match + w_rel*edge_weight − w_conflict*inconsistency`.
  - Return contexts with explicit `evidence` thought_ids.

- `ui_help`
  - Restrict to the four tools; examples reflect uniform routing and evidence-bearing responses.

**Score Fusion Details**

- `semantic`: cosine similarity against node/frame embeddings.
- `episodic`: cosine vs thoughts × `recency_decay = exp(-Δt/τ)`; τ configurable.
- `procedure_match`: boost if query intent maps to procedural frames/relations.
- `edge_weight`: accumulate weights along included edges (cap depth=1 initially).
- `inconsistency`: penalty if frames contradict (simple heuristic first: exact-opposite property values or flagged contradictions).

**Migrations**

- `schema_version(version INTEGER PRIMARY KEY, applied_at TEXT)`.
- On open, apply migration scripts idempotently; create triggers for FTS sync.
- Keep migrations identical across core/uniform DBs where applicable.

**Operational Notes**

- WAL + backups: checkpoint periodically; safe `.backup` while WAL active.
- Concurrency: single MCP process preferred; if multiple, rely on WAL + backoff.
- Encryption: consider OS-level encryption or SQLCipher build if needed.

**Testing**

- Unit tests: repository traits mocked; validate `ui_think` writes embedding and evidence; `ui_remember` fuses scores deterministically with seeded inputs.
- Integration (optional): `#[ignore]` tests creating temp uniform DBs with minimal schema; exercise FTS and vector pipeline.
- Keep stdout quiet for MCP transport.

**Cutover Strategy**

- Feature flag `UI_STORAGE=sqlite|redis` for a brief dual-write period (optional).
- Backfill: export from Redis JSON to per-uniform SQLite (include chain metadata and KG). Simple batching; compute embeddings where missing.
- Read switch: enable UniformRouter; monitor correctness/latency; keep Redis for rollback for a week.

**Risks & Mitigations**

- Vector extension availability: if `sqlite-vec` not available, use HNSW fallback.
- Knowledge fragmentation: mitigate via core attachment, cross-ref table, and promotion rules.
- Duplicate entities across uniforms: prefer local nodes; link to stable core entities with `xref` if needed.
- Performance: keep DBs small per uniform; add indexes conservatively; profile queries; cache hot kNN results in memory if needed.

**Next Steps**

1) Implement UniformRouter (open/attach, migrations, config).
2) Apply schema + FTS triggers; choose vectors backend (sqlite-vec vs HNSW) and wire helpers.
3) Update repositories to target SQLite (behind `UI_STORAGE=sqlite`).
4) Wire handlers: `ui_think` embedding + frames evidence; `ui_knowledge` frames/relations; `ui_remember` multi-view fusion.
5) Add tests; run pilot with `photography` + `legacymind` + `core`; review retrieval quality and latency.

