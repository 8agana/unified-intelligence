# Repository Guidelines

## Project Structure & Module Organization
- Source in `src/`: `main.rs` (binary) and `lib.rs` (library).
- Key modules: `handlers/` (MCP tools: `ui_think`, `ui_recall`, `ui_help`, `ui_knowledge`), plus `frameworks.rs`, `service.rs`, `repository*.rs`, `qdrant_service.rs`, `redis.rs`, `embeddings.rs`, `models.rs`, `validation.rs`, `visual.rs`, `transport.rs`.
- Config at repo root: `config.yaml` (Redis/Qdrant, rate limits, model settings).
- Tests colocated with code (e.g., `src/handlers/test_handlers.rs`).

## Build, Test, and Development Commands
- `cargo build` / `cargo build --release`: Compile in debug/release.
- `cargo run`: Run locally. Example: `GROQ_API_KEY=… OPENAI_API_KEY=… cargo run --release`.
- `cargo check`: Fast type-check without building artifacts.
- `cargo test`: Execute unit tests.
- `cargo fmt` and `cargo clippy -- -D warnings`: Format and lint; fix all warnings before PRs.

## Coding Style & Naming Conventions
- Rust style with 4-space indent.
- Names: `snake_case` for functions/modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Avoid `unwrap`/`expect` in non-test code; prefer `Result` with meaningful errors.
- Keep modules focused; avoid god files; update docs/comments when behavior changes.

## Testing Guidelines
- Use standard `#[test]` unit tests colocated with code.
- Name tests `test_*`; group with `mod tests { … }`.
- Mock/gate external deps (Redis/Qdrant, network); unit tests should not require live services.
- Run `cargo test`; target a module with `cargo test handlers::test_handlers`.

## Commit & Pull Request Guidelines
- Commits: clear, imperative subjects with prefixes like `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`; reference issues (e.g., `Fix #11`).
- PRs: include summary, rationale, testing notes; link issues; add logs/screenshots for behavior changes.
- Ensure `cargo fmt && cargo clippy -- -D warnings && cargo test` pass locally before opening.

## Security & Configuration Tips
- Never hardcode secrets; use env vars (`GROQ_API_KEY`, `OPENAI_API_KEY`) and optionally load via `dotenvy`.
- Tune Redis/Qdrant settings, similarity thresholds, and limits in `config.yaml`.
- MCP/stdio: keep stdout clean (no stray prints) to avoid handshake issues.

