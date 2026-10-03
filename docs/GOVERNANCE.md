# GOVERNANCE

```
AUTO_MERGE=false
AUTO_DEPLOY=false
PRODUCTION_WRITE=false
HIGH_RISK_REPAIR=BLOCK
MIN_CONFIDENCE=0.55
MAX_RISK=0.45
```

**Fuente de verdad de umbrales**: este archivo (0.55 / 0.45). El runtime los
replica en `worker/src/worker/mod.rs` y `scripts/verify_repo.py` comprueba que
no se separen (claim `GATE_THRESHOLDS_RUNTIME` / `GATE_THRESHOLDS_DOCS`).

Cadena de autoridad:
```
POLICY → PATCH VALIDATION → CI → VERIFY → PUSH AUTHORIZATION → main
```

## Matriz de transiciones (auditada 2026-10-02)

De donde viene la matriz: `jules-5813573718571814256-249ce1c3` (PR #7). Se
integra aqui sin sustituir las secciones ya auditadas de este archivo, que se
verificaron contra la API de GitHub y que la rama habia borrado.

| Transicion | Actor | Autoridad | Input | Output | Evidencia | Permiso | Fallback |
|---|---|---|---|---|---|---|---|
| **INCIDENT → SIGNATURE** | Edge Worker | `repair_types::FailureSignature` | `Incident` | firma + fingerprint | hash FNV-1a no criptografico | isolate de solo lectura | descartar incidente |
| **SIGNATURE → FEATURES** | Edge Worker | `feature_engine::extract` (V1) | `Incident` + firma | `FeatureVector[64]` | invariancia verificada en test | isolate de solo lectura | 64 ceros (fail-closed) |
| **FEATURES → ACTION** | Edge Worker | `RepairNet` (WEIGHT_COUNT = 2863) | `FeatureVector` | `RepairAction` (op, conf, risk) | log del isolate | WASM en el propio Worker | `model/current` → `model/stable` → `BLOCKED` (red de ceros prohibida) |
| **ACTION → GATE** | Edge Worker | `repair_operators::gate` | `RepairAction` | allow / deny + `AgentStatus` | `PipelineReport` | minimo: sin escritura | `BLOCKED` + `NeedsHuman` |
| **ACTION → PATCH** | Operadores deterministas | `repair_operators::apply` | `RepairAction` | `CandidatePatch` | allowlist de operadores | `contents: read` | escalar a humano (advisory) |
| **INCIDENT → ESTADO** | Durable Object | `IncidentState` (SQLite) | repo + incidente | veredicto dedup/quota/anti-loop | estado transa

ccional | binding `INCIDENT_STATE` | 503 `state_store_unavailable` |
| **VERDICT → COLA** | Edge Worker | Queue `REPAIR_QUEUE` | veredicto `queued` | `QueueTask` | 202 + `correlation_id` | productor de la cola | 503 `queue_unavailable` |
| **COLA → PATCH** | Consumidor asincrono | `queue_consumer` | `QueueTask` | decision + verificacion | `max_retries=3` + DLQ | consumidor | DLQ y registro en el DO |
| **GATE → PR** | GitHub App / bot | fuera de este repo hoy | `CandidatePatch` | Pull Request | diff unificado obligatorio | `contents: write`, `pull_requests: write` | **BLOCKED**: sin generador de diff no hay PR (`docs/CONTRACT.md` §3) |
| **PR → VERIFY** | GitHub Actions | autoridad de VERIFY | commit de la PR | PASS / FAIL | logs de `ci.yml` | `actions: read`, `checks: read` | CI rojo bloquea el merge |
| **VERIFY → main** | Persona maintainer | `AUTO_MERGE=false` | verdict + CI | commit | SHA en `main` | admin del repo | `git revert` |
| **main → DEPLOY** | `deploy.yml` | environment `production` | commit de `main` | Worker desplegado | log de wrangler + smoke test | `production` + required reviewers |wrangler falla y no publica |

La fila **GATE → PR** esta vacia a proposito: hoy el sistema no abre PRs.
`docs/CONTRACT.md` marca el generador de diff como P2 `[BLOCKED]`, y sin diff
unificado una PR seria ruido, no un arreglo.

## Lógica de gate (resumen)

```
if confidence < MIN_CONFIDENCE → BLOCK (LowConfidence)
if risk > MAX_RISK             → BLOCK (HighRisk)
if operator_id desconocido     → BLOCK
if WEBHOOK_SECRET no configurado → 503 (endpoint cerrado, nunca fail-open)
if WEBHOOK_SECRET incorrecto     → 401
else                           → ALLOW
```

La comparacion del secret es en tiempo constante
(`runtime::security::verify_webhook_secret`). Un secret compartido no es una
firma: `x-hub-signature-256` no se verifica y `docs/CONTRACT.md` §7 no lo afirma.

La NN solo propone. Governance + CI/VERIFY deciden.

Notas:
- Valores hot-reloadables vía KV en el futuro.


- Cada decisión se registra en RepairCase para auditoría.
- Por qué no existe fallback a LLM: `docs/NO_LLM_POLICY.md`.
- Qué está verde hoy en el flujo: `docs/E2E_CHECKLIST.md`.
- Documentación expandida de gate/metrics se mantuvo deliberadamente corta; ver DISCREPANCIES si hay conflicto con diseños previos (0.80/0.25).

## Autoridad de VERIFY (auditado 2026-10-02)

GitHub Actions es la autoridad de VERIFY, concretamente los jobs de
`.github/workflows/ci.yml`, que se ejecutan en cada push a `main` y en cada PR:

| Check | Comando | Required propuesto |
|-------|---------|---------------------|
| `workspace-test` | `cargo test --workspace` | si |
| `workspace-clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | si |
| `workspace-fmt` | `cargo fmt --all -- --check` | si (desde 2026-10-03: árbol formateado, item 34 cerrado) |
| `worker-check` | `cargo check --manifest-path worker/Cargo.toml --all-targets` y `--target wasm32-unknown-unknown --release` | si |
| `worker-test` | `cargo test --manifest-path worker/Cargo.toml` | si |
| `worker-clippy` | `cargo clippy --manifest-path worker/Cargo.toml --all-targets -- -D warnings` | si || `unused-deps` | `cargo machete` + `cargo shear` (deps declaradas sin uso / archivos sin enlazar) | si (nuevo 2026-10-03) |


Workflows que apoyan a VERIFY pero **no** son su autoridad, y por eso no deben
entrar en required checks: `consistency.yml` (deriva entre docs, codigo y
workflows), `repair-validation.yml` (mismo gate de PR, mas explicito),
`security.yml` (audit + gitleaks), `wasm.yml`, `regression.yml`.

`worker/` tiene su propio `[workspace]` (item 35): ningun comando
`--workspace`/`--all` del manifiesto raiz lo compila. Por eso existen los jobs
`worker-*` con `--manifest-path worker/Cargo.toml`.

## Merge (verificado 2026-10-02)

- `AUTO_MERGE=false` (este archivo) es la fuente de verdad: ningun merge sin
  accion humana.
- Mergify NO esta configurado (no existe `.mergify.yml`); no analizar ni
 
 documentar como si existiera. No se introduce.
- Branch protection de `main`: UNVERIFIABLE/ausente. Evidencia (2026-10-02):
  
la API `branches/main/protection` responde 401 sin token admin;
  `rulesets` devuelve `[]`; `main.protected = false`. En consecuencia NINGUN
  check es hoy obligatorio a nivel de plataforma y este documento no afirma
  que GitHub los exija: la exigencia es disciplinaria hasta que una persona
  con permisos de admin los active en Settings -> Branches -> Require status
  checks (los siete nombres exactos de la tabla de arriba).
- Ausentes por decision explicita (crear solo con pedido humano):
  `CODEOWNERS`, `dependabot.yml`, `SECURITY.md`.

## Secret legacy

- `legacy/CONFIG.md` contiene un `VERCEL_ORG_ID` literal (id de equipo de la
  era V0, archivada y no ejecutada). No se rota ni se elimina en silencio:
  decision humana. Ver "Reconciliacion P1" en DISCREPANCIES.
