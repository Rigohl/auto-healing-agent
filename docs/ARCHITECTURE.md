# Auto-Healing Agent — Architecture (Rust / WASM NN Core)

## Principle

The neural network is a **classifier / selector of repair operators**, never a code generator.

```
GitHub webhook
   ↓
Cloudflare Worker (workers-rs)     validates WEBHOOK_SECRET, normalizes
   ↓
Incident + FailureSignature
   ↓
Feature Engine                     deterministic → [f32; 64], schema v2
   ↓
Rust Neural Network (repair_nn_core)   MLP 64→32→16 + operator/confidence/risk heads
   ↓
RepairAction                       { node_id, repair_operator, parameters, confidence, risk }
   ↓
Risk / Confidence Gate             MIN_CONFIDENCE=0.55, MAX_RISK=0.45 (docs/GOVERNANCE.md)
   ↓
Deterministic Operator (repair_operators)   CandidatePatch — no free-form codegen
   ↓
GitHub PR → Actions CI/wasm = VERIFY (PASS / FAIL / BLOCKED)
   ↓
RepairCase → MongoDB (memory, diseñado) + TrainingExample (offline `repair_train`)
```

## Crates

| Crate | Role | Status |
|-------|------|--------|
| `repair_types` | `Incident`, `RepairAction`, `RepairCase`, `OperatorId` (0–12 + Unknown), `FeatureVector[64]` schema v2 | ✅ |
| `feature_engine` | `Incident` + `FailureSignature` → `[f32; 64]` | ✅ |
| `repair_nn_core` | MLP inference, `no_std` + alloc, 2863 weights | ✅ |
| `repair_nn_wasm` | wasm-bindgen adapter: `RepairModel` | ✅ (build in CI) |
| `repair_operators` | `apply()` + `gate()` deterministic stubs | ✅ partial (AST real = later) |
| `worker` | CF Worker: `/health`, `/model` (KV pointer), `/webhook` + consumidor de cola + Durable Object de estado | ✅ runtime asíncrono PART3 (webhook fail-closed → DO → Queue → pipeline) |

## Inference path (edge)

1. Cloudflare Worker receives webhook, validates secret.
2. Normalizes to `Incident`.
3. `feature_engine::extract` → `[f32; 64]`.
4. `RepairNet::predict` (or WASM `RepairModel::predictFromFeatures`) → `RepairAction`.
5. Gate on `confidence >= 0.55` / `risk <= 0.45`.
6. If actionable → `repair_operators::apply` → `CandidatePatch`.
7. Open PR; GitHub Actions verifies (PASS / FAIL / BLOCKED) — **CI is VERIFY authority, never model confidence**.
8. Persist `RepairCase`.

## Training path (offline)

Trainer offline V1 (`crates/repair_train`, deterministic SGD, no LLM) outside CF → export flat `f32[2863]` → `model/current.txt` → KV (`MODEL_KV`: `model/current` / `model/stable`).
The Worker never trains; only loads exported weights. `current.json` / `stable.json` remain metadata placeholders.

Layer arithmetic (from `crates/repair_nn_core/src/lib.rs:18`):
`64*32+32` (W1,b1) + `32*16+16` (W2,b2) + `16*13+13` (operator head)
+ `16+1` (confidence head) + `16+1` (risk head) = **2863**.

## Constraints

- Free-tier Cloudflare + MongoDB Atlas M0 + GitHub Actions.
- **No LLM in the repair loop.** Legacy HF path in `legacy/` is archived, not executed.
- WASM target: `wasm32-unknown-unknown`.
- No Pony. No free-form source generation.
- Governance: `AUTO_MERGE=false`, `AUTO_DEPLOY=false`, `PRODUCTION_WRITE=false`, `HIGH_RISK_REPAIR=BLOCK`.

## Memory layers

| Layer | Role |
|-------|------|
| Notion | Architecture, runbooks, human decisions |
| MongoDB | incidents, signatures, repair cases, audit, training examples |
| KV (`MODEL_KV`) | Model pointers (current/stable) |
| R2 | WASM artifacts / checkpoints (when real artifact exists) |
| Mem0 / D1 / DO SQLite | Deferred — not provisioned |
