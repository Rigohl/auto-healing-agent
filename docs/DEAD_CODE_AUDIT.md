# Auditoría de código muerto / sin consumidor (2026-10-08, rev. 2)

> Método: revisión manual del árbol completo (crates/, worker/, workflows/,
> scripts/, model/, docs/) + búsqueda de referencias por símbolo en GitHub.
> Criterio: un ítem es "sin consumidor" cuando ningún camino de producción
> (Worker, workflows de Actions) lo invoca en runtime o en deploy.
> Rev. 2: tras los PR #114/#122 (adopción KV + LLM fallback estructurado) y
> el PR de code-repair (operadores de edición acotada + cableado de los
> crates huérfanos), esta tabla refleja el estado post-adopción.

## Resumen

| Ítem | Consumidor en producción | Veredicto |
|------|--------------------------|-----------|
| `crates/repair_nn_wasm` | `tests/payload_check.rs` valida `model/current.txt` a través de su frontera `from_weights` en cada CI; además `wasm.yml`/`repair-validation.yml` lo compilan wasm32 | Con rol: validador del artefacto del modelo en CI |
| `crates/repair_pr` (CLI `repair-pr`) | Smoke en `repair-validation.yml`: `repair-pr diff` con fixture (exit 0) y bundle vacío → exit 2 (BLOCKED); sigue siendo además herramienta offline de evidencia | Con rol: gate de CI + evidencia offline (CONTRACT.md §3/§5) |
| `crates/repair_train` | `promote-model.yml` (input `retrain`: re-entrena antes de promover, accuracy ≥ 0.90) y `auto-repair.yml` ("Retrain NN (manual)": entrena y publica artefacto) | Con rol: bucle de re-entrenamiento del champion/challenger |
| `crates/repair_nn_core` | Worker (`model.rs` carga y predice) + `repair_nn_wasm` + `repair_train` | En producción (vía principal de decisión) |
| `repair_types::TrainingExample` | Contrato versionado por diseño (DISCREPANCIES item 74) | **No eliminar** |
| `repair_types::contract::*` (`CandidatePatch`, `OutboundPRRequest`, `VerifiedResult`, `DuplicateResponse`, `GitHubEventType`, `MinimumPermissions`, `get_error_policy`) | `repair_pr` (ahora con smoke en CI) + contrato GitHub↔Cloudflare v1 (CONTRACT.md) | Mantener |
| `model/current.json`, `model/stable.json`, `model/schema.json` | `schema.json` lo consume `scripts/verify_repo.py` (claims input_dim=64, operator_classes=13); current/stable son metadata documentada (README) | Mantener como metadatos |
| Doble `CandidatePatch` | `repair_operators::CandidatePatch` (runtime) vs `repair_types::contract::CandidatePatch` (contrato) | Duplicación de nombre con roles distintos; riesgo de confusión; documentado aquí |
| Doble generador de diff | `repair_operators::diff` (worker, find/replace acotado) vs `repair_pr::diff` (`similar`, genérico) | Divergencia posible; el smoke de CI mantiene vivo al segundo |

## KV: adopción completa (PR #114/#122)

Todos los namespaces KV del wrangler.toml tienen uso en producción:
`MODEL_KV` (pesos champion), `NN_WEIGHTS` (challenger, clave
`model/candidate`; solo reporta, `candidate.rs`), `STATE` (ledger de
firmas + replay, `ledger.rs`), `CACHE` (circuit breaker, `circuit.rs`),
`AGENT_CONFIG` (config en caliente + presupuesto LLM `llm/budget`,
`config_store.rs`/`llm_fallback.rs`), `REPAIR_CASES_KV` (casos con
`origin` para distilación). Ya no hay namespaces huérfanos.

## Detalle de adopciones (rev. 2)

### 1. `repair_train` → bucle de re-entrenamiento
Antes: CLI sin consumidor en workflows. Ahora:
- `promote-model.yml` input `retrain=true` ejecuta
  `cargo run -p repair_train --release -- --out model/current.txt` antes
  del preflight (umbral interno accuracy ≥ 0.90, fail-closed).
- `auto-repair.yml` reescrito como "Retrain NN (manual)": entrena, valida
  2863 tokens y publica el payload como artefacto de Actions. La promoción
  a KV sigue siendo un dispatch humano aparte (GOVERNANCE.md).
- El input `also_candidate=true` de `promote-model.yml` sube además el
  payload a `NN_WEIGHTS:model/candidate`, cerrando el flujo
  champion/challenger que Sourcery señalaba desconectado.

### 2. `repair_nn_wasm` → validador del artefacto
Antes: solo canario de build wasm32 redundante. Ahora:
`tests/payload_check.rs` carga `model/current.txt` y instancia
`RepairModel::from_weights` en cada `cargo test --workspace`: si el
artefacto deja de cargar por la frontera pública JS, CI falla antes del
promote. Sigue compilando wasm32 como canario.

### 3. `repair_pr` → smoke de evidencia en CI
Antes: sin consumidor. Ahora: `repair-validation.yml` ejecuta
`repair-pr diff` con un fixture inline (bundle esperado) y verifica que
un input vacío sale con exit 2 (BLOCKED, CONTRACT.md §3: nunca diffs
inventados). El CLI de PR real (`repair-pr pr`) sigue siendo manual.

### 4. `auto-repair.yml` → Retrain NN (manual)
Antes: solo `echo` de la política legacy NO_LLM. Ahora: workflow manual
de re-entrenamiento con publicación de artefacto. Sin escritura de
producción automática.

## Sobre lo que NO es código muerto

- `RepairCase` / `VerificationResult::Skipped`: consumidos por
  `persist_case` (REPAIR_CASES_KV).
- `escalate` / operadores advisory (EnvVarRepair, CacheClear): política
  deliberada (LLM_POLICY.md), no death code.
- `RETRY_DELAY_SECONDS` fijo: el contador propio anterior se eliminó
  (era inalcanzable); el hard stop vive en el DO `/attempt`.
- Operadores de edición acotada de `repair_operators::diff`
  (SYNTAX_FIX, IMPORT_PATH_FIX, SOURCE_REPAIR): ahora alcanzables también
  vía LLM fallback (propuesta `file`/`from`/`to` validada fail-closed en
  `llm_fallback.rs`), siempre ejecutados por el operador determinista.

## Estado del deploy verificado (2026-10-08)

- Worker `auto-healing-agent` desplegado y activo en Cloudflare
  (id 7c6b8ec9534b4e97bafa2e2fa79de066, modificado 2026-10-07).
- Namespaces KV del wrangler.toml existen y todos tienen consumidor
  (ver sección KV arriba).
