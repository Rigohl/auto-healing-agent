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
  repair_train/   # trainer offline V1 (bin repair-train)
worker/          # Cloudflare Worker (workers-rs)
model/           # current.txt = payload KV real (2863 f32)
docs/            # incluye PROMPT_PAD.md v2
scripts/
```

## Comandos

```bash
cargo test -p repair_types -p feature_engine -p repair_nn_core -p repair_operators
cargo check --manifest-path worker/Cargo.toml   # worker/ va fuera del workspace
cargo build -p repair_nn_wasm --target wasm32-unknown-unknown
cargo run -p repair_train --release -- --out model/current.txt   # re-entrenar V1
```

`model/current.txt` es el **payload KV real** (2863 pesos `f32` en texto) del
entrenamiento V1: `worker/src/worker/model.rs` carga exactamente ese formato
desde `MODEL_KV` (clave `model/current`). `model/current.json` y
`model/stable.json` siguen siendo placeholders de metadata (`weights: null`,
nadie los consume). La promoción de pesos a KV es una acción humana: ver
`docs/E2E_CHECKLIST.md`.

## Prompt operativo

Ver [`docs/PROMPT_PAD.md`](docs/PROMPT_PAD.md).
