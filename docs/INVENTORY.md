# INVENTORY — FASE 1 (2026-09-30)

Fuente: árbol GitHub API `main` @ `6a8dc525` + lectura de archivos clave. Notion: página "auto-healing-agent — Rust/WASM Neural Network Core" (id `3eb50567-40be-8146-b75a-e90f6f818580`, editada 12:52 UTC) y duplicado `(1)` (id `3eb50567-40be-8078-aeeb-ea1bcda6abe1`, editada 16:19 UTC).

## Árbol completo (63 entradas)

```
auto-healing-agent/
├── .github/
│   └── workflows/
│       ├── auto-repair.yml      (659 B)  DISABLED legacy LLM
│       ├── ci.yml               (361 B)  cargo test 4 crates
│       ├── regression.yml        (288 B)  weekly cron + manual
│       ├── repair-validation.yml  (199 B)  nota: CI = VERIFY authority
│       ├── security.yml          (213 B)  hygiene
│       └── wasm.yml             (410 B)  build repair_nn_wasm
├── Cargo.toml               (648 B)  workspace, 5 members
├── README.md                (966 B)
├── rust-toolchain.toml      (103 B)  stable + wasm32-unknown-unknown
├── crates/
│   ├── feature_engine/       extract() → FeatureVector[64], schema v2
│   ├── repair_nn_core/        RepairNet 64→32→16 + heads, WEIGHT_COUNT=2617
│   ├── repair_nn_wasm/        RepairModel wasm-bindgen
│   ├── repair_operators/      apply() + gate() stubs
│   └── repair_types/         Incident, RepairAction, RepairCase, OperatorId 0-12+255
├── docs/                   12 archivos (ver abajo)
├── legacy/                 agent.ts, worker.js, CONFIG, SECRETS, README, setup
├── model/                  current.json (placeholder), schema.json v0.2.0, stable.json
├── scripts/                set-github-secrets.sh único
└── worker/                 workers-rs + wrangler.toml (KV MODEL_KV)
```

## Archivos clave leídos

| Archivo | Hallazgo |
|---------|----------|
| `Cargo.toml` | workspace members: 5 crates. `worker/` excluido (package CF separado). Deps: serde, serde_json. **Sin `Cargo.lock` en el repo.** |
| `worker/wrangler.toml` | name `auto-healing-agent-worker`, build `worker-build`, **solo** `[[kv_namespaces]] MODEL_KV` con IDs placeholder. Sin R2, sin D1, sin Durable Objects, sin routes. |
| `worker/Cargo.toml` | `worker = "0.5"`, `console_error_panic_hook`. LTO + opt-level s. |
| `worker/src/lib.rs` | Router: `GET /`, `/health`, `/model` (lee KV pointer), `POST /webhook` (valida `WEBHOOK_SECRET`, acepta, no despacha). **No llama a repair_nn_wasm ni feature_engine.** |
| `crates/repair_types` | `OperatorId` 0..12 (13 ops) + Unknown=255. `FeatureVector` DIM=64, SCHEMA_VERSION=2. `RepairAction { node_id, repair_operator, parameters, confidence, risk }`. `is_actionable(min_c, max_r)`. |
| `crates/feature_engine` | 32 features pobladas (0..31), 32..63 = hash determinista del fingerprint + schema version + bias. Test con fixture `syntax_error`/`vercel-build`. |
| `crates/repair_nn_core` | MLP 64→32→16, soft-argmax sobre 13 ops, heads confidence/risk con sigmoid. `WEIGHT_COUNT = 2617`. `no_std` + alloc. Tests: zeros_predicts, weight_count_stable. |
| `crates/repair_nn_wasm` | `RepairModel::new()` (zeros), `fromWeights`, `predictFromFeatures` (exige len==64), `weightCount`, `featureDim`. |
| `crates/repair_operators` | `apply()` match por OperatorId → CandidatePatch (stubs; AST real = later). `gate(action, min_c, max_r)` → PipelineReport. **No existe `should_fallback_to_llm`.** |
| `model/schema.json` | v0.2.0: input 64, hidden 32, latent 16, 13 clases, heads operator/confidence/risk. |
| `model/current.json` | placeholder `0.1.0-placeholder`, weights null. |
| `model/stable.json` | `0.0.0-none`, weights null. |
| `.github/workflows/ci.yml` | `cargo test -p repair_types -p feature_engine -p repair_nn_core -p repair_operators` (en push/PR a main). |
| `.github/workflows/wasm.yml` | build `--target wasm32-unknown-unknown` en push a crates wasm. |
| `.github/workflows/auto-repair.yml` | solo `workflow_dispatch` + echo; LLM path deshabilitado. |
| `.github/workflows/repair-validation.yml` | solo echo: "CI is VERIFY authority". |
| `.github/workflows/regression.yml` | cron semanal + manual. |
| `.github/workflows/security.yml` | solo echo hygiene. |
| `docs/GOVERNANCE.md` | `MIN_CONFIDENCE=0.55`, `MAX_RISK=0.45`, AUTO_MERGE/DEPLOY/PRODUCTION_WRITE=false, HIGH_RISK_REPAIR=BLOCK. |
| `docs/AGENTS.md` | 14 roles lógicos (Incident..DevOps), sin LLM, sin Pony. |
| `docs/PROMPT_PAD.md` | v2: SoT dims = 64; behavioral rule 2: si conflicto de dims, usar 64. |
| `docs/DISCREPANCIES.md` | ya existe en repo; dice "Source of truth dims: Prompt Pad / LAB = 64 (no el snippet Notion 16)". |
| `docs/PHASE_STATUS.md` | Fase 1 completed; 2-7 partial; 8 omitted; 9 not green. |
| `legacy/agent.ts` | path LLM HuggingFace + MongoDB postmortems + Linear + Vercel + Exa. ~300 líneas. |
| `legacy/worker.js` | gateway JS: detecta Vercel/Linear, valida `x-webhook-secret`, despacha `repository_dispatch` `auto-repair`. |
| `scripts/set-github-secrets.sh` | sube secrets legacy (HF, EXA, VERCEL, LINEAR, GH_PAT, MONGDB_URI). |

## Ausencias confirmadas (API 404)

- `package.json` (cero resultados en code search) → **no hay Node/TS runtime en el repo**.
- `tests/` (directorio) → tests viven inline `#[cfg(test)]` en los crates.
- `scripts/train_model.rs`, `export_weights.rs`, `validate_model.rs`.
- `model/README.md`.
- `docs/HYBRID_POLICY.md` → **Notion lo cita como documentado; el repo no lo tiene.**
- `LICENSE`, `Cargo.lock`.
- Dependencias de memoria: **cero** menciones de Mem0, R2, D1, Durable Objects, MongoDB driver en código Rust.

## Lenguajes (API)

Rust 64.5% · TypeScript 30.0% · JavaScript 12.6% · Shell 9.6% (el TS/JS es casi todo `legacy/`).

## PR / ramas

- PR #1 (`feat/rust-nn-core`) **merged** 2026-09-30 12:51 UTC. Solo rama visible: `main`.
- 6 workflows activos en Actions.

## Notion — contenido canónico de la página

- Arquitectura: webhook → Worker → Incident → features → NN WASM → RepairAction → gate → operador → PR → VERIFY → RepairCase → MongoDB.
- Memoria: Notion (arquitectura), MongoDB (incidents/signatures/cases/audit/training), Mem0 (cuando el conector exista), R2 (artefactos WASM/checkpoints), KV (config/pointers), DO SQLite (estado/lock), D1 (metadatos).
- Estructura propuesta incluye `tests/{neural,operators,fixtures,regression}/`, `scripts/{train,export,validate}_model.rs`, `model/README.md`, `LICENSE`, `Cargo.lock`.
- Snippet de referencia NN: **16→12→5** outputs. Sección "Estado de implementación" (tabla de crates) describe MLP **16→32→11, 907 pesos** — desactualizado respecto al repo (64→32→16, 2617 pesos).
- Política híbrida: "NN primero; si no actionable → agent.ts + HuggingFace"; cita `docs/HYBRID_POLICY.md`.
- Estado MongoDB observado: colecciones `audit`, `incidents`, `postmortems`, `repair_rules`, `training_examples`; incidente histórico `syntax_error` con 3 intentos, `repair_failed`.
- Fuentes: Context7 Burn + wasm-bindgen.

## Decisión de esta fase

Repo = fuente de verdad del código. Notion = diseño + decisiones humanas, **pero su sección de estado de implementación está desfasada** (dims 16 vs 64, 907 vs 2617 pesos, política híbrida vs rust-only). Toda discrepanción tiene decisión en `docs/DISCREPANCIES.md`.
