# CODEMAP v3 — post implementación A–E en main

**Rama:** `main` only · **Pony:** out · **LLM núcleo:** prohibido

## Pipeline

```
Incident → FailureSignature → features[64] → RepairNet
  → RepairAction → gate → CandidatePatch → PR → Actions VERIFY
```

## Crates vivos

| Crate | Rol |
|-------|-----|
| repair_types | FailureSignature, RepairAction V0, FeatureVector 64 |
| feature_engine | extract() |
| repair_nn_core | 64→32→16 + heads |
| repair_operators | apply + gate |
| repair_nn_wasm | wasm-bindgen |
| worker/ | CF health + MODEL_KV (deploy aparte) |

## Local

```bash
cargo test -p feature_engine -p repair_nn_core -p repair_operators
cargo build -p repair_nn_wasm --target wasm32-unknown-unknown
```

## Siguiente

1. AST operators reales
2. Pesos entrenados (Burn offline)
3. Deploy worker + KV real
4. Desconectar path HF de producción
