ui_remember configuration

- Key: `ui_remember.hybrid_weights`
  - `semantic`: weight for vector similarity (0.0–1.0)
  - `text`: weight for keyword/text match (0.0–1.0)
  - `recency`: weight for temporal recency boost (0.0–1.0)

- Presets: `ui_remember.preset`
  - `balanced-default`: semantic 0.60, text 0.25, recency 0.15
  - `fast-chat`: semantic 0.45, text 0.40, recency 0.15
  - `deep-research`: semantic 0.75, text 0.10, recency 0.15
  - `recall-recent`: semantic 0.45, text 0.15, recency 0.40

Environment overrides
- `UI_REMEMBER_PRESET` overrides `ui_remember.preset`.
- Weights override keys:
  - `UI_REMEMBER_WEIGHT_SEMANTIC`
  - `UI_REMEMBER_WEIGHT_TEXT`
  - `UI_REMEMBER_WEIGHT_RECENCY`

Resolution order
1) File `config.yaml` is loaded
2) Env overrides are applied
3) Preset is applied last (overwrites any provided weights)

Example YAML
```
ui_remember:
  preset: balanced-default
  hybrid_weights:
    semantic: 0.6
    text: 0.25
    recency: 0.15
```

Example env preset
```
UI_REMEMBER_PRESET=deep-research
```

Local verification commands
```
cargo fmt --all -- --check
cargo clippy -- -D warnings
cargo test
# optional
cargo audit
```
