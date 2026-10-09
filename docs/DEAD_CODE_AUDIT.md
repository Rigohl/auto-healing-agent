# Auditoria de codigo muerto / sin consumidor (2026-10-08)

> Metodo: revision manual del arbol completo (crates/, worker/, workflows/,
> scripts/, model/, docs/) + busqueda de referencias por simbolo en GitHub.
> Criterio: un item es "sin consumidor" cuando ningun camino de produccion
> (Worker, workflows de Actions) lo invoca en runtime o en deploy.

## Resumen

| Item | Consumidor en produccion | Veredicto |
|------|--------------------------|-----------|
| crates/repair_nn_wasm | repair-validation.yml compila el artefacto wasm32, cargo test --workspace ejecuta tests/payload_check.rs (carga model/current.txt via from_weights + prediccion real) Y el job node-boundary de wasm.yml EJERCE los exports JS (weightCount, featureDim, predictFromFeatures, operator_name) bajo Node con wasm-bindgen-test | Con uso desde 2026-10-08: valida el payload KV contra la API publica antes del promote; la superficie JS es real y verificada |
| crates/repair_pr (CLI repair-pr) | repair-validation.yml ejecuta repair-pr diff sobre un fixture en cada PR | Con uso desde 2026-10-08: puente de evidencia verificado en CI; el camino de produccion sigue siendo github_client.rs |
| crates/repair_train (CLI repair-train) | regression.yml (job trainer-canary, semanal) entrena V1 y valida el payload (umbral 90%); promote-model.yml con retrain=true re-entrena, y con export_examples=true ademas aprende de los TrainingExample reales exportados de REPAIR_CASES_KV | Con uso desde 2026-10-08: eslabon visible de la destilacion + loop de aprendizaje real; promover el payload a KV sigue siendo accion humana |
| repair_types::TrainingExample | queue_consumer::persist_case congela las features en training_example:{id} (REPAIR_CASES_KV); /github/callback marca verified/reward con la senal REAL de Actions; repair-train --examples entrena con ellos (promote-model.yml export_examples) | Con uso desde 2026-10-08 (loop de aprendizaje PART4 cerrado). Antes era contrato sin consumidores (DISCREPANCIES item 74) |
| repair_types::contract::CandidatePatch, OutboundPRRequest, VerifiedResult, DuplicateResponse, GitHubEventType, MinimumPermissions, get_error_policy | repair_pr (verificado en CI) + tests de contrato (tests/contract/mod.rs). El Worker usa compute_idempotency_key e IDEMPOTENCY_TTL_SECONDS; la regla de evidencia de VerifiedResult se aplica en /github/callback (PASS sin evidencia => 400) | Contrato GitHub-Cloudflare v1 (CONTRACT.md) con uso creciente. Mantener |
| model/current.json, model/stable.json, model/schema.json | Ninguno (weights: null; el payload real es model/current.txt -> KV model/current) | Placeholders de metadata documentados (README). Mantener como metadatos o eliminar |
| Doble CandidatePatch | repair_operators::CandidatePatch (runtime) vs repair_types::contract::CandidatePatch (contrato) | Duplicacion de nombre con roles distintos: riesgo de confusion; documentado aqui |
| Doble generador de diff | repair_operators::diff (worker, package.json) vs repair_pr::diff (similar, generico) | Divergencia posible: el runtime usa el primero; el CLI es evidencia offline |
| repair_operators::apply + PipelineReport del gate + SyntheticIncident.error_category | PART6 (2026-10-08): apply() genera el "Operator plan" del PR y del preview /webhook; la razon del gate viaja al DO como nn_gate_denied:{detalle}; error_category alimenta el accuracy por categoria del trainer | Reproposito con uso real desde PART6 (explainability) |

## Detalle y recomendaciones

### 1. repair_nn_wasm
El Worker enlaza repair_nn_core como crate Rust dentro de su propio modulo
WASM (worker/Cargo.toml), por lo que repair_nn_wasm (frontera
wasm-bindgen) no tiene consumidor de runtime dentro del Worker: ningun
codigo del worker importa RepairModel/JsRepairAction. Desde el 2026-10-08
(toma la opcion b de esta auditoria) la superficie publica JS es un
producto REAL y verificado: tests/js_boundary.rs ejerce weightCount,
featureDim, predictFromFeatures y operator_name bajo Node (wasm-pack
test --node, job node-boundary de wasm.yml), ademas del payload_check
nativo que valida model/current.txt antes de cada promote. Un consumidor
JS externo (dashboard embebido, herramientas de terceros) tiene hoy una
frontera estable y testeada.

### 2. repair_pr
CLI con dos modos (diff, pr). El camino de produccion (queue_consumer ->
github_client) NO lo usa: abre rama + commit + PR via REST con worker::Fetch.
repair_pr queda como herramienta de evidencia offline y como puente
CONTRACT.md §3/§5 con octocrab. repair-validation.yml ejecuta repair-pr
diff en cada PR (usado).
**Riesgo**: dos implementaciones paralelas de "abrir PR de reparacion"
(github_client.rs vs repair_pr/src/github.rs) y dos generadores de diff
pueden divergir en silencio; CI no compara sus salidas.
**Recomendacion**: mantener repair-pr diff como verificacion de CI sobre
parches de ejemplo (ya conectado); no asumir que el modo pr esta en el
path de produccion.

### 3. Placeholders de model/
current.txt (2863 f32) es el payload KV real que model.rs carga desde
MODEL_KV:model/current. current.json, stable.json y schema.json no los
lee nadie (ya documentado en README e INDEX). Sin accion urgente.

### 4. Sobre lo que NO es codigo muerto
- RepairCase / VerificationResult::Skipped: consumidos por persist_case
  (REPAIR_CASES_KV).
- TrainingExample: consumido por persist_case + /github/callback +
  repair-train --examples (loop PART4).
- escalate / operadores advisory (EnvVarRepair, CacheClear): politica
  deliberada (LLM_POLICY.md), no death code.
- RETRY_DELAY_SECONDS fijo: el contador propio anterior se elimino
  (era inalcanzable); el hard stop vive en el DO /attempt.

### 5. Loop de aprendizaje real (PART4, cerrado 2026-10-08)
persist_case (worker) congela las features del momento de la reparacion
en training_example:{correlation_id} porque el RepairCase NO guarda el
Incidente completo y las features no son reconstruibles despues.
/github/callback marca verified/reward con la senal REAL de Actions (un
PASS sin evidencia se rechaza 400: regla VerifiedResult). promote-model.yml
con retrain + export_examples exporta el JSONL y repair-train --examples
mezcla esa senal con el sintetico usando el MISMO nucleo SGD (sin caminos
paralelos). Solo la senal verificada positiva entrena: un caso FAIL o
pendiente jamas ensena su etiqueta como buena.

## Estado del deploy verificado (2026-10-08)
- Worker auto-healing-agent desplegado y activo en Cloudflare
  (id 7c6b8ec9534b4e97bafa2e2fa79de066, modificado 2026-10-07).
- Los namespaces KV del wrangler.toml existen en la cuenta:
  MODEL_KV (73014a1b...) y REPAIR_CASES_KV (996211a0...).
- Los namespaces STATE, CACHE, agent-config y neural-net-weights fueron
  ADOPTADOS por el Worker en el PR #114 (kv-adoption, 2026-10-08): STATE ->
  ledger (replay de firmas PASS), CACHE -> circuit breaker, agent-config ->
  config_store (config en caliente), neural-net-weights -> NN_WEIGHTS
  (challenger). El registro "sin uso" de arriba quedo desactualizado con
  ese merge; esta linea lo corrige.

## 6. Reproposito PART6 (2026-10-08): explainability

Los tres items "muertos" del analisis profundo dejan de estarlo con uso
real de produccion (misma regla de siempre: no se borra, se cablea):
- `repair_operators::apply()` construye el "Operator plan" (summary /
  steps / files / advisory) que se incluye en el preview del webhook
  (respuesta 202) y en el cuerpo del PR de reparacion: el revisor
  humano lee QUE hara el operador antes de leer el diff. Tambien queda
  en el patch_summary del RepairCase (auditoria via dashboard).
- El `PipelineReport` del gate deja de descartarse: la denegacion de la
  NN llega al DO como `nn_gate_denied:c=... r=... op=...` en vez de un
  "gate_denied" opaco.
- `SyntheticIncident.error_category` (feature_engine) gana consumidor:
  `repair_train::evaluate` reporta accuracy POR CATEGORIA y el CLI la
  imprime antes del promote (un 90% global puede esconder una categoria
  al 50%).
- El modo `repair-pr pr` se verifica fail-closed en CI (guarda de rama
  no efimera, sin git/red/token): repair-validation.yml.


## 7. Higiene PART7 (2026-10-08): poda de API muerta y docs obsoletos

Regla general del repo: no se borra, se cablea. Excepcion honesta para dos
wrappers de contract.rs sin NINGUN consumidor de produccion y sin posible
repurpose no-contradictorio:
- `RepairEvent::idempotency_key()`: duplicaba la fuente canonica
  `compute_idempotency_key` (que el worker YA usa directamente en
  /webhook). Mantener dos rutas hacia la misma clave era riesgo de drift.
  Eliminado; tests ajustados a la funcion canonica.
- `MinimumPermissions::default_required()`: constructor muerto; la lista
  de permisos minimos es contrato de DATOS (docs/CONTRACT.md) y el struct
  sigue reexportado y verificado (CONTRACT_REEXPORTS_MinimumPermissions).
- Docs obsoletos eliminados: MEM0_ANALYSIS.md, MEM0_STATUS.md,
  TOkyo_Night_COMPATIBILITY.md (analisis de una arquitectura pre-DO ya
  superada). INDEX.md corregido: link roto NO_LLM_POLICY.md -> LLM_POLICY.md
  (roto desde el PR #122) y fila de MEM0_STATUS eliminada.
- Falso positivo aclarado: repair_pr/src/diff.rs y
  repair_operators/src/diff.rs NO son duplicados (bundle `similar` vs
  FileEdit del gate): abstracciones distintas, ambos con uso real.
- `repair_nn_wasm` (exports JS): frontera WASM testeada por diseño
  (js_boundary + payload_check); se mantiene.
