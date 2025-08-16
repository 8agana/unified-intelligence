# Repository Guidelines

## Definitions
- **Framework (WorkflowState):** Top-level operating mode selected by the user/system: `conversation` (default), `debug`, `build`, `stuck`, `review`. One active at a time; guides interaction style.
- **ThinkingMode:** Sub-layer analysis technique within a framework: `first_principles`, `ooda`, `systems`, `root_cause`, `swot`. Multiple can be active; internal lenses/voices surfaced when useful.
- **ThinkingSet:** A set of ThinkingModes active within the current framework. Backed by `EnumSet<ThinkingMode>`, serializes as a transparent JSON array with deterministic ordering.

## Project Structure & Module Organization
- Source in `src/`: `main.rs` (binary) and `lib.rs` (library).
- MCP handlers in `src/handlers/`: `ui_think`, `ui_recall`, `ui_help`, `ui_knowledge`.
- Supporting modules: `frameworks.rs`, `service.rs`, `repository*.rs`, `redis.rs`, `embeddings.rs`, `models.rs`, `validation.rs`, `visual.rs`, `transport.rs`.
- Config at repo root: `config.yaml` (Redis/Qdrant, rate limits, model settings).
- Tests are colocated with code, e.g., `src/handlers/test_handlers.rs`.

## Build, Test, and Development Commands
- `cargo check`: Fast type-check without building artifacts.
- `cargo build` / `cargo build --release`: Compile in debug/release.
- `GROQ_API_KEY=… OPENAI_API_KEY=… cargo run --release`: Run locally with required env.
- `cargo test`: Run unit tests. Scope examples: `cargo test handlers::test_handlers`.
- `cargo fmt` and `cargo clippy -- -D warnings`: Format and lint; fix all warnings before PRs.

## Coding Style & Naming Conventions
- Rust style with 4-space indent; prefer idiomatic constructs.
- Names: `snake_case` (functions/modules), `CamelCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- Avoid `unwrap`/`expect` in non-test code; return `Result` with meaningful errors.
- Keep modules focused; update docs/comments when behavior changes.

## Remote MCP (HTTP)
- Transport: `rmcp` streamable HTTP served by Axum at `UI_HTTP_PATH` (default `/mcp`).
- Enable: `UI_TRANSPORT=http`; bind via `UI_HTTP_BIND` (e.g., `127.0.0.1:8787`).
- Auth: set `UI_BEARER_TOKEN` or pass `?access_token=TOKEN` in the URL for headerless clients.
- Claude Desktop: paste `https://<host>/mcp?access_token=TOKEN`.
- Cloudflare: map hostname to the local bind (e.g., `mcp.samataganaphotography.com -> http://localhost:8787`).

## Current MCP Tools
- `ui_think`: Capture and process thoughts with optional chaining and frameworks.
- `ui_recall`: Retrieve a single thought by ID or all thoughts in a chain.
- `ui_help`: Usage, parameters, and examples for tools/frameworks.
- `ui_knowledge`: CRUD + relations for a Redis-backed knowledge graph.
- `ui_context`: Store short-lived personal/federation context with embeddings.
- `ui_memory`: Search/read/update/delete memory using RediSearch + filters.
- `ui_remember`: Conversational memory with assistant synthesis and feedback metrics.

## Redis Data Model
- **Current state:** Mixed usage of Redis types — RedisJSON (thoughts/entities), Hashes (context/memory indexes), Strings/Binary (embedding cache), and Streams (events).
- **RediSearch:** Presently targets Hash records; Lua helpers sometimes fetch JSON docs after search.
- **Guidance:** For new features, prefer RedisJSON for primary records; keep type usage consistent within a feature.
- **Migration option:** Consider standardizing to RedisJSON-only with JSON-backed RediSearch indexes; add a background migrator for existing Hash data.
- **Action item:** Document key schemas and decide standardization timeline before expanding memory features.

## Known Issues
- Build currently fails on `src/frameworks.rs` due to type/variant mismatches (`ThinkingFramework` vs `ThinkingMode`, variant casing). Fix planned on branch `framework-refactor`.
- Some docs referenced files have been reorganized; Groq integration now lives in `transport.rs`, `intent.rs`, and `synth.rs`.

## Next Steps
- Create and use a worktree/branch `framework-refactor` to correct framework enums, naming, and handler integration, then restore `cargo check`/tests and clippy to green.

## Deferred Work (Future Iteration)
- Stuck cycling: Wire `StuckTracker` into `ui_think` to persist and rotate ThinkingModes across calls when framework_state=stuck (store per chain in Redis).
- User-steered modes: Optionally accept `thinking_mode` or `thinking_set` overrides (soft hints) with forgiving parsing; keep default behavior fully automatic.
- Telemetry: Count non-canonical `framework_state` inputs that required synonym/fuzzy mapping for later tuning, without interrupting users.
- Priority/ranking: Re-introduce priority helpers if we need to rank modes or states in retrieval or display. Removed for now to keep code surface minimal.

## Testing Guidelines
- Use `#[test]` unit tests colocated with code; group under `mod tests`.
- Name tests `test_*`; mock/gate external deps (Redis, Qdrant, network).
- Unit tests should not require live services; run with `cargo test`.
- Keep stdout clean to avoid MCP/stdio handshake issues.

## Commit & Pull Request Guidelines
- Commits: imperative, concise subjects; prefixes `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`; reference issues (e.g., `Fix #11`).
- PRs: include summary, rationale, testing notes; link issues; add logs/screenshots for behavior changes.
- Before opening: ensure `cargo fmt && cargo clippy -- -D warnings && cargo test` all pass locally.

## Security & Configuration Tips
- Never hardcode secrets; use env vars (`GROQ_API_KEY`, `OPENAI_API_KEY`), optionally via `dotenvy`.
- Tune Redis settings and limits in `config.yaml`.
