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
RepairCase → KV `REPAIR_CASES_KV` (V1; Mongo diseñado, sin driver WASM) + TrainingExample (offline `repair_train`)
```

## Crates

| Crate | Role | Status |
|-------|------|--------|
| `repair_types` | `Incident`, `RepairAction`, `RepairCase`, `OperatorId` (0–12 + Unknown), `FeatureVector[64]` schema v2 | ✅ |
| `feature_engine` | `Incident` + `FailureSignature` → `[f32; 64]` | ✅ |
| `repair_nn_core` | MLP inference, `no_std` + alloc, 2863 weights | ✅ |
| `repair_nn_wasm` | wasm-bindgen adapter: `RepairModel` | ✅ (build in CI) |
| `repair_operators` | `apply()` + `gate()` + `diff` (edit acotado → unified diff determinista) | ✅ (AST real = later) |
| `repair_pr` | GATE→PR V1: unified diff real (`similar`) + PR opener (`octocrab`), bin `repair-pr` | ✅ V1 CLI offline — superseded en el path del worker por `repair_operators::diff` + `worker/src/worker/github_client.rs` (sin `octocrab`) |
| `worker` | CF Worker (Axum): `/health`, `/model`, `/model/candidate` (pointer KV), `/webhook`, `/github/callback`, `/dashboard` + consumidor de cola + Durable Object de estado + MONITOR (cron) | ✅ runtime asíncrono PART3: webhook fail-closed → DO → Queue → pipeline → PR real vía `github_client.rs` |

## Inference path (edge)

1. Cloudflare Worker receives webhook, validates secret.
2. Normalizes to `Incident`.
3. `feature_engine::extract` → `[f32; 64]`.
4. `RepairNet::predict` (or WASM `RepairModel::predictFromFeatures`) → `RepairAction`.
5. Gate on `c
onfidence >= 0.55` / `risk <= 0.45`.
6. If actionable → `repair_operators::apply` → `CandidatePatch`; `worker/src/worker/param_derive.rs` deriva `dependency`/`version` de forma determinista (fail-closed).
7. If actionable, the worker builds the bounded edit + unified diff (`repair_operators::diff`) and opens the PR via `github_client.rs` (GitHub REST con `worker::Fetch`, sin `octocrab`, fail-closed: errores permanentes = Blocked, transitorios = retry de cola → DLQ); GitHub Actions verifies (PASS / FAIL / BLOCKED) and reports to `POST /github/callback` — **CI is VERIFY authority, never model confidence**.
8. Persist `RepairCase` in `REPAIR_CASES_KV` y `TrainingExample` (con research web persistida en el DO) para el reentrenamiento offline (best-effort tras abrir el PR).

## Training path (offline)

Trainer offline V1 (`crates/repair_train`, deterministic SGD, no LLM) outside CF → export flat `f32[2863]` → `model/current.txt` → KV (`MODEL_KV`: `model/current` / `model/stable`).
The Worker never trains; only loads exported weights. PART8 (2026-10-08): `model/current` / `model/stable` en `MODEL_KV` apuntan a pesos reales (`current.json` / `stable.json` ya no son placeholders); promover `stable` es acción humana (GOVERNANCE.md).

Layer arithmetic (from `crates/repair_nn_core/src/lib.rs:18`):
`64*32+32` (W1,b1) + `32*16+16` (W2,b2) + `16*13+13` (operator head)
+ `16+1` (confidence head) + `16+1` (risk head) = **2863**.

## Constraints

- Free-tier Cloudflare + MongoDB Atlas M0 + GitHub Actions.
- LLM **acotado como fallback tras el gate** (decisión del dueño 2026-10-08, PR #122): `worker/src/worker/llm_fallback.rs` vía Workers AI; la salida se valida contra los operadores deterministas y se destila a `TrainingExample` para reentrenar la NN (docs/LLM_POLICY.md). Sin generación libre de código por LLM. El path HF V0 (antiguo `legacy/`) se eliminó el 2026-10-05.
- WASM target: `wasm32-unknown-unknown`.
- No Pony. No free-form source generation.
- Governance: `AUTO_MERGE=false`, `AUTO_DEPLOY=false`, `PRODUCTION_WRITE=false`, `HIGH_RISK_REPAIR=BLOCK`.

## Memory layers

| Layer | Role |
|-------|------|
| Notion | Architecture, runbooks, human decisions |
| MongoDB | incidents, signatures, repair cases, audit, training examples |
| KV (`MODEL_KV`) | Model pointers (current/stable) |
| KV (`REPAIR_CASES_KV`) | RepairCases persistidos (V1: alternativa a MongoDB sin driver WASM) |
| R2 | WASM artifacts / checkpoints (when real artifact exists) |
| Mem0 | Descartado del runtime (PART7, 2026-10-08): el worker no lo llama. D1: deferred |
| DO SQLite | **Provisioned** (binding `INCIDENT_STATE`, clase `IncidentState`) |
