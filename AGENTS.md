# Repository Guidelines

## Project Structure & Module Organization
- Source lives in `src/`: binary in `main.rs`, library in `lib.rs`.
- MCP handlers in `src/handlers/`: `ui_think`, `ui_recall`, `ui_help`, `ui_knowledge`.
- Supporting modules: `frameworks.rs`, `service.rs`, `repository*.rs`, `redis.rs`, `embeddings.rs`, `models.rs`, `validation.rs`, `visual.rs`, `transport.rs`.
- Config at repo root: `config.yaml` (Redis/Qdrant, rate limits, model settings).
- Tests are colocated with code (e.g., `src/handlers/test_handlers.rs`).

## Build, Test, and Development Commands
- Type check: `cargo check`.
- Build: `cargo build` (or `--release`).
- Run locally: `GROQ_API_KEY=… OPENAI_API_KEY=… cargo run --release`.
- HTTP MCP server (optional): `UI_TRANSPORT=http UI_HTTP_BIND=127.0.0.1:8787 UI_BEARER_TOKEN=TOKEN cargo run --release` (serves at `UI_HTTP_PATH` default `/mcp`).
- Tests: `cargo test` (scope: `cargo test handlers::test_handlers`).
- Format & lint: `cargo fmt` and `cargo clippy -- -D warnings`.

## Coding Style & Naming Conventions
- Rust, 4‑space indentation; prefer idiomatic constructs.
- Naming: `snake_case` (functions/modules), `CamelCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- Avoid `unwrap`/`expect` in non‑test code; return `Result` with meaningful errors.
- Keep modules focused; update docs/comments when behavior changes.
- Data model: prefer RedisJSON for new features; keep type usage consistent per feature.

## Testing Guidelines
- Use `#[test]` unit tests colocated with code under `mod tests`.
- Name tests `test_*`; keep stdout clean to avoid MCP/stdio handshake issues.
- Mock/gate external deps (Redis, Qdrant, network) so `cargo test` runs offline.
- Example: `cargo test handlers::test_handlers`.

## Commit & Pull Request Guidelines
- Commits: imperative subjects with prefixes `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`; reference issues (e.g., `Fix #11`).
- Before PR: ensure `cargo fmt && cargo clippy -- -D warnings && cargo test` all pass.
- PRs: include summary, rationale, testing notes; link issues; add logs/screenshots for behavior changes.

## Security & Configuration Tips
- Never hardcode secrets; use env vars (`GROQ_API_KEY`, `OPENAI_API_KEY`).
- Tune Redis/Qdrant and limits in `config.yaml`.
- HTTP auth: set `UI_BEARER_TOKEN` or use `?access_token=TOKEN`.
- Cloudflare: map a hostname to `UI_HTTP_BIND` (e.g., `mcp.example.com -> http://localhost:8787`).

