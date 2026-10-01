# Auto-Healing Agent — Rust / WASM only

Núcleo de reparación: **Rust + WebAssembly + NN pequeña** en Cloudflare (Always Free).

- La red **no escribe código** → `RepairAction` estructurada
- Operadores deterministas aplican el parche
- **GitHub Actions** = VERIFY (nunca la confidence del modelo)
- **Sin TypeScript / sin LLM** en el path de producción
- **Sin Pony**

## Estructura (Notion + Prompt Pad)

```
crates/          # código Rust — abrir aquí en móvil
  repair_types/
  feature_engine/
  repair_nn_core/
  repair_nn_wasm/
  repair_operators/
worker/          # Cloudflare Worker (workers-rs)
model/
docs/            # incluye PROMPT_PAD.md v2
scripts/
legacy/          # archivo muerto (agent.ts no se ejecuta)
```

## Comandos

```bash
cargo test -p repair_types -p feature_engine -p repair_nn_core -p repair_operators
cargo check --manifest-path worker/Cargo.toml   # worker/ va fuera del workspace
cargo build -p repair_nn_wasm --target wasm32-unknown-unknown
```

`model/current.json` y `model/stable.json` son **placeholders** (`weights: null`).
Todavía no existe entrenamiento ni export de pesos: ver `docs/E2E_CHECKLIST.md`.

## Prompt operativo

Ver [`docs/PROMPT_PAD.md`](docs/PROMPT_PAD.md).
