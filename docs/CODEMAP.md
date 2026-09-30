# CODEMAP — Guía de implementación (canónica)

**Repo:** `Rigohl/auto-healing-agent`  
**Rama persistente única:** `main`  
**Pony:** ignorado (no forma parte del plan activo)  
**LLM libre:** prohibido en el núcleo de decisión/parche

Este documento **guía el código** y reutiliza:
- Notion *Rust/WASM Neural Network Core* + *SYSTEM PROMPT Agent Health*
- Prompt Pad (Always Free)
- `docs/ARCHITECTURE.md`, `docs/DISCREPANCIES.md` (cuando existan en main)

---

## 1. Principio (no negociable)

```
Incident → Evidence → FailureSignature
  → Features[64] → NN (WASM) → RepairAction
  → Policy gate → Operador determinista → PR
  → GitHub Actions VERIFY (autoridad)
  → RepairCase / TrainingExample
```

La red **solo clasifica**. No escribe fuentes.

---

## 2. Mapa path → rol → estado

| Path | Rol | Estado en main |
|------|-----|----------------|
| `worker.js` | Gateway legacy webhooks → dispatch | **vivo** (legacy) |
| `agent.ts` | Cerebro LLM HF | **legacy V0** — no ampliar |
| `.github/workflows/auto-repair.yml` | Orquesta agent.ts | **legacy** |
| `crates/repair_types` | Incident, OperatorId, FeatureVector, RepairAction | **scaffold** (revisar dim 16 vs 64) |
| `crates/feature_engine` | Incident → features | **scaffold 16-dim** |
| `crates/repair_nn_core` | MLP inferencia | **scaffold 16→32→11** |
| `crates/repair_nn_wasm` | wasm-bindgen | **puede faltar en main** — añadir |
| `crates/repair_operators` | Operadores deterministas | **puede faltar en main** — añadir |
| `worker/` | Worker Rust workers-rs | **pendiente** |
| `model/schema.json` | Contrato features/ops | **presente** |
| `docs/ARCHITECTURE.md` | Arquitectura | **presente** |
| `docs/CODEMAP.md` | **Esta guía** | **canónica** |
| MongoDB | incidents, postmortems, training_examples | **cuenta Atlas** |

---

## 3. Orden de trabajo (un solo hilo en main)

### Paso A — Contratos (tipos)
1. Subir `RepairAction` a `{ node_id, repair_operator, parameters, confidence, risk }`.
2. Añadir `FailureSignature`.
3. `FeatureVector::DIM = 64` + `SCHEMA_VERSION`.

### Paso B — Encoder + NN
1. `feature_engine::extract(incident, signature) -> [f32; 64]`.
2. `repair_nn_core`: 64→32→16 + heads operator/confidence/risk; `from_weights` sin panic (`Result`).
3. Tests con fixture `syntax_error` / `buildStep` / `npm run vercel-build`.

### Paso C — Operadores + gate
1. `repair_operators::apply` + `gate` → `blocked_by_policy` | `needs_human` (nunca HF).
2. Stubs allowlisted primero; AST real después.

### Paso D — WASM + CI
1. `repair_nn_wasm` (solo adaptador; core sin wasm-bindgen).
2. Workflows: `ci.yml`, `wasm.yml` (verify = Actions, no confidence).

### Paso E — Worker
1. `worker/` con workers-rs: health + secret + lectura `MODEL_KV`.
2. Sustituir gradualmente `worker.js`.
3. Límites free: sin train, sin build de repo en edge.

### Paso F — Aprendizaje offline
1. `TrainingExample` al PASS de CI.
2. Burn fuera de CF → export pesos → `model/current|stable`.

---

## 4. Legacy (no borrar aún; no crecer)

| Archivo | Uso |
|---------|-----|
| `agent.ts` | Solo hasta que operadores + NN den CandidatePatch real |
| `worker.js` | Hasta deploy Worker Rust |
| `auto-repair.yml` | Hasta workflow NN-first |

Cualquier feature nueva va al path Rust, no a HF.

---

## 5. Agentes lógicos (sin Pony)

Roles = módulos/fases, no procesos LLM:

Incident → Evidence → FailureSignature → Localization → NeuralPolicy  
→ Governance → PatchOperator → Verification → Learning → Registry → DevOps

Contratos detallados: ampliar en `docs/AGENTS.md` cuando se cree en main.

---

## 6. Git

- **Solo `main` persistente.**
- Ramas `feat/*`, `fix/*`, `setup/*`: **borrar** tras integrar.
- PR: ref efímera → merge → borrar ref.

Comandos (local / `gh`):

```bash
gh api -X DELETE repos/Rigohl/auto-healing-agent/git/refs/heads/feat/rust-nn-core
gh api -X DELETE repos/Rigohl/auto-healing-agent/git/refs/heads/feat/rust-wasm-nn
gh api -X DELETE repos/Rigohl/auto-healing-agent/git/refs/heads/fix/safejson-null-payload
gh api -X DELETE repos/Rigohl/auto-healing-agent/git/refs/heads/setup/secrets
```

---

## 7. Criterio de hecho (mínimo viable)

- [ ] `cargo test` en repair_types + feature_engine + repair_nn_core
- [ ] features 64 + RepairAction completo
- [ ] gate sin path LLM
- [ ] wasm build en CI
- [ ] Worker Rust health 200
- [ ] Un incidente fixture → RepairAction → CandidatePatch (stub) documentado

---

## 8. Fuentes técnicas

- workers-rs: https://github.com/cloudflare/workers-rs
- wasm-bindgen: https://github.com/wasm-bindgen/wasm-bindgen
- Burn (train offline): https://github.com/tracel-ai/burn
- CF limits: https://developers.cloudflare.com/workers/platform/limits/

---

*Actualizar este archivo en cada merge a main. Es el mapa operativo del código.*
