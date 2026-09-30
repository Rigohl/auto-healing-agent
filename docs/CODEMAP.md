# CODEMAP — orden del código

## Abrir en el móvil / GitHub

1. Carpeta **`crates`** → cada subcarpeta → **`src`** → **`lib.rs`** (código Rust)
2. Carpeta **`worker`** → **`src`** → **`lib.rs`**
3. **`docs`** = solo documentación

## Workspace

```
Cargo.toml
crates/repair_types
crates/feature_engine
crates/repair_nn_core
crates/repair_nn_wasm
crates/repair_operators
worker/   (CF, build con wrangler)
```

## Flujo de código

```
repair_types::Incident
  → FailureSignature::from_incident
  → feature_engine::extract → [f32; 64]
  → repair_nn_core::RepairNet::predict → RepairAction
  → repair_operators::gate + apply → CandidatePatch
```

WASM: `repair_nn_wasm::RepairModel`  
Edge: `worker` Router `/health` `/model` `/webhook`

## Sin Pony. Sin generación libre LLM en el núcleo.
