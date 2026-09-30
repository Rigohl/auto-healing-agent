# Auto-Healing Agent

Núcleo **Rust + WASM + NN** (Always Free). La red **no escribe código**.

Estructura alineada a Notion (*Rust/WASM Neural Network Core*):

```
auto-healing-agent/
├─ crates/                 ← código Rust (abrir aquí en el móvil)
│   ├─ repair_types/
│   ├─ feature_engine/
│   ├─ repair_nn_core/
│   ├─ repair_nn_wasm/
│   └─ repair_operators/
├─ worker/                 ← Cloudflare Worker Rust
├─ model/
├─ docs/
├─ scripts/
├─ .github/workflows/
├─ legacy/                 ← LLM path (no ampliar)
├─ Cargo.toml
├─ rust-toolchain.toml
└─ README.md
```

## Código (no es solo Markdown)

En GitHub móvil: **crates** → p.ej. **repair_nn_core** → **src** → **lib.rs**

```bash
cargo test -p feature_engine -p repair_nn_core -p repair_operators
```

## Legacy

`legacy/agent.ts` + workflow `auto-repair.yml` = ruta HuggingFace antigua.
