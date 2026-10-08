# Auditoría de código muerto / sin consumidor (2026-10-08)

> Método: revisión manual del árbol completo (crates/, worker/, workflows/,
> scripts/, model/, docs/) + búsqueda de referencias por símbolo en GitHub.
> Criterio: un ítem es "sin consumidor" cuando ningún camino de producción
> (Worker, workflows de Actions) lo invoca en runtime o en deploy.

## Resumen

| Ítem | Consumidor en producción | Veredicto |
|------|--------------------------|----------|
| `crates/repair_nn_wasm` | `repair-validation.yml` compila el artefacto wasm32 Y `cargo test --workspace` ejecuta `tests/payload.rs` (carga `model/current.txt` vía `from_weights` + predicción real) | Con uso desde 2026-10-08: valida el payload KV contra la API pública antes del promote |
| `crates/repair_pr` (CLI `repair-pr`) | `repair-validation.yml` ejecuta `repair-pr diff` sobre un fixture en cada PR | Con uso desde 2026-10-08: puente de evidencia verificado en CI; el camino de producción sigue siendo `github_client.rs` |
| `crates/repair_train` (CLI `repair-train`) | `regression.yml` (job `trainer-canary`, semanal) entrena V1 y valida el payload (umbral 90%) | Con uso desde 2026-10-08: eslabón visible de la distilación; promover el payload a KV sigue siendo acción humana |
| `repair_types::TrainingExample` | Ninguno | Contrato versionado por diseño (DISCREPANCIES item 74). **No eliminar** |
| `repair_types::contract::CandidatePatch`, `OutboundPRRequest`, `VerifiedResult`, `DuplicateResponse`, `GitHubEventType`, `MinimumPermissions`, `get_error_policy` | Solo `repair_pr` (que a su vez no tiene consumidor). El Worker usa `compute_idempotency_key` e `IDEMPOTENCY_TTL_SECONDS` del mismo módulo | Contrato GitHub↔Cloudflare v1 (CONTRACT.md). Mantener; revisar si `repair_pr` se elimina |
| `model/current.json`, `model/stable.json`, `model/schema.json` | Ninguno (`weights: null`; el payload real es `model/current.txt` → KV `model/current`) | Placeholders de metadata documentados (README). Mantener como metadatos o eliminar |
| Doble `CandidatePatch` | `repair_operators::CandidatePatch` (runtime) vs `repair_types::contract::CandidatePatch` (contrato) | Duplicación de nombre con roles distintos: riesgo de confusión; documentado aquí |
| Doble generador de diff | `repair_operators::diff` (worker, package.json) vs `repair_pr::diff` (`similar`, genérico) | Divergencia posible: el runtime usa el primero; el CLI es evidencia offline |

## Detalle y recomendaciones

### 1. `repair_nn_wasm`
El Worker enlaza `repair_nn_core` como crate Rust dentro de su propio módulo
WASM (worker/Cargo.toml), por lo que `repair_nn_wasm` (frontera
wasm-bindgen) no tiene consumidor de runtime: ningún código importa
`RepairModel`/`JsRepairAction` fuera de la propia crate. Hoy su única
función es que `wasm.yml` compile `cargo build -p repair_nn_wasm --target
wasm32-unknown-unknown` como canario. Como `worker-check` ya compila el
worker real a wasm32, el canario es redundante.
**Opciones**: (a) eliminar la crate y el job de build de `wasm.yml`, dejando
`worker-check` como única verificación wasm32; (b) mantenerla como superficie
pública JS documentada. Decisión pendiente del dueño; hoy no rompe nada.

### 2. `repair_pr`
CLI con dos modos (`diff`, `pr`). El camino de producción (queue_consumer →
github_client) NO lo usa: abre rama + commit + PR vía REST con `worker::Fetch`.
`repair_pr` queda como herramienta de evidencia offline y como puente
CONTRACT.md §3/§5 con octocrab. Ningún workflow lo ejecuta.
**Riesgo**: dos implementaciones paralelas de "abrir PR de reparación"
(github_client.rs vs repair_pr/src/github.rs) y dos generadores de diff
pueden divergir silenciosamente; CI no compara sus salidas.
**Recomendación**: o se conecta `repair-pr diff` como verificación de CI
sobre parches de ejemplo, o se marca la crate como herramienta de auditoría
manual en su README para evitar que se asuma que está en el path de producción.

### 3. Placeholders de `model/`
`current.txt` (2863 f32) es el payload KV real que `model.rs` carga desde
`MODEL_KV:model/current`. `current.json`, `stable.json` y `schema.json`
no los lee nadie (ya documentado en README e INDEX). Sin acción urgente.

### 4. Sobre lo que NO es código muerto
- `RepairCase` / `VerificationResult::Skipped`: consumidos por
  `persist_case` (REPAIR_CASES_KV).
- `escalate` / operadores advisory (EnvVarRepair, CacheClear): política
  deliberada (NO_LLM_POLICY.md), no death code.
- `RETRY_DELAY_SECONDS` fijo: el contador propio anterior se eliminó
  (era inalcanzable); el hard stop vive en el DO `/attempt`.

## Estado del deploy verificado (2026-10-08)
- Worker `auto-healing-agent` desplegado y activo en Cloudflare
  (id 7c6b8ec9534b4e97bafa2e2fa79de066, modificado 2026-10-07).
- Los namespaces KV del wrangler.toml existen en la cuenta:
  MODEL_KV (73014a1b…) y REPAIR_CASES_KV (996211a0…).
- Los namespaces STATE, CACHE, agent-config y neural-net-weights fueron
  ADOPTADOS por el Worker en el PR #114 (kv-adoption, 2026-10-08): STATE ->
  ledger (replay de firmas PASS), CACHE -> circuit breaker, agent-config ->
  config_store (config en caliente), neural-net-weights -> NN_WEIGHTS
  (challenger). El registro "sin uso" de arriba quedó desactualizado con
  ese merge; esta línea lo corrige.
