# Auto-Healing Agent — Architecture (Rust / WASM NN Core)

## Principle

The neural network is a **classifier / selector of repair operators**, never a code generator.

```
Incident
   ↓
Feature Engine          (16-dim deterministic features)
   ↓
Rust Neural Network     (MLP 16→32→11, pure arithmetic)
   ↓
RepairAction            { operator_id, confidence, risk }
   ↓
Risk / Confidence Gate
   ↓
Deterministic Operator  (patch logic)
   ↓
Candidate Patch → GitHub PR → VERIFY → RepairCase
```

## Crates

| Crate | Role |
|-------|------|
| `repair_types` | `Incident`, `RepairAction`, `RepairCase`, `OperatorId`, `FeatureVector` |
| `feature_engine` | `Incident` → `[f32; 16]` |
| `repair_nn_core` | MLP inference, `no_std` + alloc ready |
| `repair_nn_wasm` | (next) wasm-bindgen adapter |
| `repair_operators` | (next) deterministic patch applicators |

## Inference path (edge)

1. Cloudflare Worker receives webhook.
2. Normalizes to `Incident`.
3. Calls WASM module → `feature_engine::extract` + `RepairNet::predict`.
4. Gate on `confidence` / `risk`.
5. If actionable → run corresponding deterministic operator.
6. Open PR; GitHub Actions verifies (PASS / FAIL / BLOCKED).
7. Persist `RepairCase` to MongoDB / Notion.

## Training path (offline)

Burn (or any trainer) → export flat `f32[907]` → `model/current.json` / `stable.json`.
The Worker never loads Burn; only the exported weights.

## Constraints

- Free-tier Cloudflare + MongoDB Atlas M0 + GitHub Actions.
- No LLM in the repair loop.
- WASM target: `wasm32-unknown-unknown` (no WASI required for core).
