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
- **Entrenamiento**: trainer propio offline V1 (`crates/repair_train`, SGD determinista, sin `rand` ni LLM). Burn sigue como referencia de diseño. Export del payload plano a `model/current.txt`.
- Alternativas evaluadas: tract, MicroFlow (solo referencia).

## Separación Training / Inference
Cloudflare ejecuta **solo** inferencia. Train/eval ocurre offline → export del payload plano → `model/current.txt` → KV (`model/current` / `model/stable`).

## Contrato mínimo repair_nn_core

```rust
#![no_std]
extern crate alloc;

pub fn infer(features: &[f32; 64], weights: &Weights) -> Prediction;
// Prediction { operator_id, confidence, risk }
```

## Objetivo de entrenamiento (conceptual, offline)

Recuperado del documento de diseño en PDF de PART1–4. **Implementado en V1**
(2026-10-03): `crates/repair_train` entrena con CE sobre la cabeza de operador y
BCE sobre las cabezas de conf/risk (`DISCREPANCIES` 5, 6, 22 cerrados). Los
términos `L_location`/`L_compile`/etc. del diseño quedan como criterio
conceptual para fases con señal real.

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

Los λ están fijados en V1 (`λ_conf = λ_risk = 0.5`, `crates/repair_train`):
cierra la decisión abierta de FASE 5.

> Nota: `RepairCase.reward` y `TrainingExample.reward` existen como `f32` en
> `crates/repair_types/src/lib.rs` pero **nada en el repo las calcula**. Esta
> fórmula es el único sitio donde se define el criterio.

## Fases relacionadas
FASE 2 (features 64), FASE 3 (NN + operators), FASE 5 (learning offline).

## Fuentes
Burn, wasm-bindgen, papers de Neural Program Repair / TinyML WASM.
