# PART 2 – Red Neuronal y Stack Tecnológico

> **Principio crítico**: La red **NO escribe código**. Solo clasifica/selecciona operador.

## Separación fundamental

```
Incident → Feature Engine → Rust Neural Network → RepairAction
         → Risk/Confidence Gate → Deterministic Operator → Patch
```

## Salida estructurada

```json
{ "operator_id": 2, "confidence": 0.87, "risk": 0.13 }
```

El operador determinista aplica el cambio real.

## Arquitectura V0 (repo actual)

| Capa | Tamaño |
|------|--------|
| Input | 64 |
| Hidden | 32 |
| Latent | 16 |
| Operator Head | 13 (OperatorId 0–12) |
| Confidence Head | 1 |
| Risk Head | 1 |
| WEIGHT_COUNT | 2863 |

## Stack
- **Inferencia**: repair_nn_core (no_std + alloc) → repair_nn_wasm.
- **Entrenamiento**: Burn offline (fuera de CF). Export de pesos planos a model/.
- Alternativas evaluadas: tract, MicroFlow (solo referencia).

## Separación Training / Inference
Cloudflare ejecuta **solo** inferencia. Train/eval ocurre offline → export_weights → model/current.json → KV/R2 pointer.

## Contrato mínimo repair_nn_core

```rust
#![no_std]
extern crate alloc;

pub fn infer(features: &[f32; 64], weights: &Weights) -> Prediction;
// Prediction { operator_id, confidence, risk }
```

## Fases relacionadas
FASE 2 (features 64), FASE 3 (NN + operators), FASE 5 (learning offline).

## Fuentes
Burn, wasm-bindgen, papers de Neural Program Repair / TinyML WASM.
