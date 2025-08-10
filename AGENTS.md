# Repository Guidelines

## Project Structure & Module Organization
- Source: `src/` with a binary entry (`main.rs`) and library (`lib.rs`).
- Key modules: `handlers/` (MCP tools like `ui_think`, `ui_recall`, `ui_help`, `ui_knowledge`), `frameworks.rs`, `service.rs`, `repository*.rs`, `qdrant_service.rs`, `redis.rs`, `embeddings.rs`, `models.rs`, `validation.rs`, `visual.rs`, `transport.rs`.
- Config: `config.yaml` at repo root for Redis/Qdrant, rate limits, and model settings.
- Tests: Unit tests live next to code (e.g., `src/handlers/test_handlers.rs`).

## Build, Test, and Development Commands
- Build (debug/release): `cargo build` / `cargo build --release`.
- Run locally: `cargo run` (or run built binary at `target/release/unified-intelligence`).
- Tests: `cargo test` (fast check: `cargo check`).
- Lint/format: `cargo clippy -- -D warnings` and `cargo fmt`.
- Example env run: `GROQ_API_KEY=... OPENAI_API_KEY=... cargo run --release`.

## Coding Style & Naming Conventions
- Rust style: 4-space indent, `snake_case` for functions/modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Formatting: enforce with `cargo fmt` before PRs.
- Linting: fix Clippy issues; avoid `unwrap/expect` in non-test code—prefer `Result` with meaningful errors.
- File layout: keep feature logic in focused modules; avoid god files.

## Testing Guidelines
- Framework: standard Rust `#[test]` with unit tests colocated; prefer small, deterministic tests.
- External deps: mock or gate network/storage paths; don’t require live Redis/Qdrant for unit tests.
- Naming: test fns `test_*`; group with `mod tests { ... }`.
- Run: `cargo test`; add focused runs via `cargo test module::name`.

## Commit & Pull Request Guidelines
- Commits: use clear, imperative subjects; conventional prefixes are common here (e.g., `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`). Reference issues like `Fix #11` when applicable.
- PRs: include summary, rationale, and testing notes; link issues; add logs/screenshots for behavior changes; ensure `cargo fmt && cargo clippy && cargo test` pass.

## Security & Configuration Tips
- Secrets: never hardcode API keys; use environment vars (`GROQ_API_KEY`, `OPENAI_API_KEY`) or `.env` loaded via `dotenvy`. Review `config.yaml` for tunables (similarity thresholds, limits).
- Protocol/MCP: server communicates over stdio; keep stdout clean (no stray prints) to avoid MCP handshake issues.
