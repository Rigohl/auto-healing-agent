# CODEMAP — orden del código

## Abrir en el móvil / GitHub

1. Carpeta **`crates`** → cada subcarpeta → **`src`** → **`lib.rs`** (código Rust)
2. Carpeta **`worker`** → **`src`** → **`lib.rs`** (router Axum) → **`src/worker/`** (módulos)
3. **`docs`** = solo documentación

## Workspace

```
Cargo.toml
crates/repair_types
crates/feature_engine
crates/repair_nn_core
crates/repair_nn_wasm
crates/repair_operators
crates/repair_train   (trainer offline, bin repair-train)
crates/repair_pr      (CLI offline: diff con similar + PR con octocrab; el path del worker NO lo usa)
worker/   (CF, build con wrangler)
```

## Edge — worker (workers-rs, Axum)

Rutas: `/health` `/model` `/model/candidate` `/webhook` `/github/callback` `/dashboard`

Módulos de `worker/src/worker/`:

- `anti_loop.rs` — guarda anti-loop de reparaciones
- `candidate.rs` — registro de candidatos de reparación
- `circuit.rs` — circuit breaker de reparaciones
- `config_store.rs` — configuración persistida del DO
- `dashboard.rs` — dashboard (HTML/JSON, token-gated)
- `github_client.rs` — GitHub REST real (rama auto-heal/* → commit → PR), sin octocrab
- `incident_state.rs` — Durable Object de estado por repositorio
- `ledger.rs` — ledger de auditoría de decisiones
- `llm_fallback.rs` — fallback LLM acotado vía Workers AI (docs/LLM_POLICY.md)
- `model.rs` — carga de pesos del modelo desde MODEL_KV
- `monitor.rs` — MONITOR (cron): retención y health
- `param_derive.rs` — derivación determinista de dependency/version (fail-closed)
- `queue_consumer.rs` — pipeline de cola → gate → diff → PR → callback
- `quota.rs` — cuota de reparaciones
- `rules.rs` — reglas/guardas de reparación
- `security.rs` — validación de secretos y tokens
- `self_guard.rs` — guarda OWN_REPO: el agente nunca se auto-edita
- `web_research.rs` — research web persistida para TrainingExample

## Flujo de código

```
repair_types::Incident
  → FailureSignature::from_incident
  → feature_engine::extract → [f32; 64]
  → repair_nn_core::RepairNet::predict → RepairAction
  → repair_operators::gate + apply → CandidatePatch (plan de operador)
  → repair_operators::diff (edit acotado + unified diff)
  → guards del worker (self_guard / rules / quota / circuit / anti_loop)
  → si el gate deniega o no hay candidato → llm_fallback (Workers AI, acotado)
  → worker github_client (rama auto-heal/* → commit → PR) → Actions VERIFY
  → POST /github/callback registra el veredicto en el DO
```

WASM: `repair_nn_wasm::RepairModel`

## Sin Pony. Sin generación libre LLM en el núcleo. LLM solo como fallback acotado (docs/LLM_POLICY.md).
