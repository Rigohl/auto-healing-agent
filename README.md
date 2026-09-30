# Auto-Healing Agent

Núcleo **Rust + WASM + red neuronal pequeña** en Cloudflare (Always Free).  
La red **no escribe código**: produce `RepairAction`; operadores deterministas aplican el parche; **GitHub Actions** verifica.

**Rama única:** `main` · **Pony:** fuera de alcance

---

## Dónde está el código (no solo Markdown)

En la app de GitHub, entra en estas carpetas:

```
crates/
  repair_types/src/lib.rs      ← tipos (Incident, RepairAction, features 64)
  feature_engine/src/lib.rs    ← encoder de features
  repair_nn_core/src/lib.rs    ← red neuronal (inferencia)
  repair_nn_wasm/src/lib.rs    ← adaptador WASM
  repair_operators/src/lib.rs  ← operadores + policy gate
worker/
  src/lib.rs                   ← Cloudflare Worker (Rust)
  wrangler.toml
```

Los `.md` en `docs/` son guías. El motor está en **`crates/**/*.rs`** y **`worker/src/lib.rs`**.

---

## Pipeline

```
Incident → FailureSignature → features[64] → NN (WASM)
  → RepairAction { node_id, operator, parameters, confidence, risk }
  → gate → operador determinista → PR → Actions VERIFY
```

---

## Build / test

```bash
cargo test -p feature_engine -p repair_nn_core -p repair_operators
cargo build -p repair_nn_wasm --target wasm32-unknown-unknown
```

Worker (cuando KV esté configurado):

```bash
cd worker && npx wrangler dev
```

---

## Legacy (no ampliar)

| Archivo | Estado |
|---------|--------|
| `agent.ts` | LLM HuggingFace — **legacy** |
| `worker.js` | Gateway JS — **legacy** hasta deploy de `worker/` |
| `auto-repair.yml` | Orquesta el LLM — **legacy** |

---

## Docs

Ver [`docs/INDEX.md`](docs/INDEX.md).
