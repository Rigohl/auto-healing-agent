# CODEMAP — Guía de implementación (canónica v2)

**Repo:** `Rigohl/auto-healing-agent`  
**Rama:** solo `main`  
**Pony:** fuera de alcance activo  
**Actualizado:** 2026-09-30 · Notion + Context7 cotejados

## Fuentes de verdad (orden)

1. Notion *SYSTEM PROMPT — Agent Health* (autoridad)
2. Notion *AUTO-REPAIR LAB* + *Rust/WASM Neural Network Core*
3. Este CODEMAP (mapa de código en repo)
4. `docs/ARCHITECTURE.md`

Si hay conflicto de dimensión NN: **Prompt Pad / LAB = 64-dim** gana sobre el snippet temprano 16/12/5 de Notion.

---

## Pipeline (no negociable)

```
GitHub source
  → Incident → Evidence → FailureSignature
  → Parser → AST+CFG+DFG (cuando exista)
  → Feature Encoder [64]
  → Rust NN (WASM)
  → RepairAction { node_id, repair_operator, parameters, confidence, risk }
  → Governance gate
  → Deterministic AST Operator → Candidate Patch
  → PR → Actions (compile/test/regression) = VERIFY authority
  → Reward → RepairCase → TrainingExample
  → Offline Burn → Checkpoint → R2/KV → Edge inference
```

NN **no** genera código. CI **no** se sustituye por confidence.

---

## NN V0 (cerrado)

| Capa | Tamaño |
|------|--------|
| Input | 64 |
| Hidden | 32 |
| Latent | 16 |
| Operator Head | K (13 clases iniciales 0..12) |
| Confidence Head | 1 |
| Risk Head | 1 |

Loss conceptual: λ_location + λ_operator + λ_compile + λ_test + λ_semantic + λ_risk  
Reward: compile + tests_fixed + regression_free + structural_validity − patch_size − risk

---

## Mapa path → estado (main)

| Path | Rol | Estado |
|------|-----|--------|
| `worker.js` | webhook → dispatch | legacy vivo |
| `agent.ts` | LLM HF | **legacy — no crecer** |
| `auto-repair.yml` | orquesta LLM | legacy |
| `crates/repair_types` | contratos | scaffold **16-dim** → migrar 64 |
| `crates/feature_engine` | encoder | scaffold 16 |
| `crates/repair_nn_core` | MLP | scaffold 16→32→11 |
| `crates/repair_nn_wasm` | adaptador | **faltante en main** |
| `crates/repair_operators` | ops + gate | **faltante en main** |
| `worker/` | workers-rs | **faltante** |
| `model/*` | schema/pesos | schema 16 legacy |
| `docs/CODEMAP.md` | esta guía | **canónica** |

---

## Context7 — patrones oficiales a copiar

### workers-rs (KV / secret / fetch)

```rust
use worker::*;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();
    router
        .get("/", |_req, _ctx| Response::ok("AUTO-REPAIR LAB"))
        .get("/health", |_req, _ctx| Response::ok("ok"))
        .get("/model", |_req, ctx| {
            // MODEL_KV binding — pointer only, not full dataset
            let kv = ctx.kv("MODEL_KV")?;
            // kv.get("model/current").text().await ...
            Response::ok("model_ptr")
        })
        .run(req, env)
        .await
}
```

- Bindings: `env.kv("MODEL_KV")`, `env.secret("WEBHOOK_SECRET")`
- **No** train, **no** repo build, **no** dataset en edge
- Límites Free: ~10 ms CPU/req, 128 MB, 100k req/día, 50 subrequests, 64 MiB bundle

### wasm-bindgen

```rust
#[wasm_bindgen]
pub struct RepairModel { /* net */ }

#[wasm_bindgen]
impl RepairModel {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RepairModel { /* zeros */ }

    #[wasm_bindgen(js_name = fromWeights)]
    pub fn from_weights(weights: &[f32]) -> Result<RepairModel, JsValue> { /* ... */ }

    #[wasm_bindgen(js_name = predictFromFeatures)]
    pub fn predict_from_features(&self, features: &[f32]) -> Result<JsRepairAction, JsValue> { /* dim==64 */ }
}
```

Core (`repair_nn_core`) **sin** `wasm_bindgen`. Solo el adaptador.

---

## Cloudflare split

| Sí en CF | No en CF |
|----------|----------|
| webhook, router, features, inference, gate, APIs ligeras | train, build repo, tests largos, dataset, git full |
| KV: model/current|stable|previous, flags, thresholds | |
| R2: wasm, weights, checkpoints | |
| DO: locks / estado incidente (si hace falta) | |

---

## 14 agentes (lógicos = módulos, no LLM)

| # | Agente | Output |
|---|--------|--------|
| 1 | Incident | Incident |
| 2 | Evidence | logs/stack |
| 3 | Repository Analyst | AST/CFG/DFG summary |
| 4 | Failure Signature | FailureSignature + hist |
| 5 | Localization | node_id[] |
| 6 | Neural Repair Policy | RepairAction |
| 7 | Patch Operator | CandidatePatch |
| 8 | Verification | PASS/FAIL from CI |
| 9 | Review | risk/diff notes |
| 10 | Learning | TrainingExample |
| 11 | Training | checkpoint (offline) |
| 12 | Model Registry | CURRENT/STABLE |
| 13 | Governance | allow/deny |
| 14 | DevOps | PR coord (no override CI) |

Cada uno: mission, I/O, tools, limits, timeout, evidence, success/block, idempotency, audit.

---

## Orden de código en main (Pasos)

| Paso | Qué | Hecho cuando |
|------|-----|--------------|
| **A** | FailureSignature + RepairAction V0 + FeatureVector 64 | tipos compilan |
| **B** | extract 64 + NN 64→32→16 heads | tests fixture syntax_error |
| **C** | operators + gate (sin HF) | blocked_by_policy path |
| **D** | repair_nn_wasm + ci.yml + wasm.yml | CI verde wasm |
| **E** | worker/ workers-rs + MODEL_KV | /health 200 |
| **F** | TrainingExample + export weights | schema v2 + script |

AST/CFG/DFG real = incrementos tras C stubs.

---

## Notion cotejado (IDs útiles)

- Rust/WASM Neural Network Core  
- AUTO-REPAIR LAB — investigación integrada  
- SYSTEM PROMPT Agent Health  
- Sistema Autónomo Rust/WASM  

Mem0: no operativo → no fingir. Elicit/Boltz: no usados para arquitectura.

---

## Criterio MVP

- [ ] cargo test types/engine/nn
- [ ] dim 64 + action completa
- [ ] gate sin LLM
- [ ] wasm en CI
- [ ] Worker health
- [ ] fixture → RepairAction → CandidatePatch stub

---

## Git

Solo `main`. Borrar ramas huérfanas con `gh api -X DELETE .../git/refs/heads/<name>`.

---

*Actualizar CODEMAP en cada cambio estructural a main.*
