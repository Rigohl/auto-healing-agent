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

## Objetivo de entrenamiento (conceptual, offline)

Recuperado del documento de diseño en PDF de PART1–4. **No implementado**:
no hay trainer en el repo y `model/*.json` sigue con `weights: null`
(`DISCREPANCIES` 5, 6, 22). Se documenta para que FASE 5 no redefina el criterio.

Loss ponderada por término:

```
L = λ_location·L_location   + λ_operator·L_operator
  + λ_compile·L_compile    + λ_test·L_test
  + λ_semantic·L_semantic  + λ_risk·L_risk
```

Reward por `RepairCase` verificada:

```
reward = compile_success + tests_fixed + regression_free
       + structural_validity − patch_size − risk
```

Los λ no están fijados: es la decisión abierta de FASE 5.

> Nota: `RepairCase.reward` y `TrainingExample.reward` existen como `f32` en
> `crates/repair_types/src/lib.rs` pero **nada en el repo las calcula**. Esta
> fórmula es el único sitio donde se define el criterio.

## Fases relacionadas
FASE 2 (features 64), FASE 3 (NN + operators), FASE 5 (learning offline).

## Fuentes
Burn, wasm-bindgen, papers de Neural Program Repair / TinyML WASM.
