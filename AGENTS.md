# Repository Guidelines

## Project Structure & Module Organization
- Source in `src/`: `main.rs` (binary) and `lib.rs` (library).
- MCP handlers in `src/handlers/`: `ui_think`, `ui_recall`, `ui_help`, `ui_knowledge`.
- Supporting modules: `frameworks.rs`, `service.rs`, `repository*.rs`, `qdrant_service.rs`, `redis.rs`, `embeddings.rs`, `models.rs`, `validation.rs`, `visual.rs`, `transport.rs`.
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
- Tune Redis/Qdrant settings, similarity thresholds, and limits in `config.yaml`.
