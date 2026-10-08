# Politica LLM — fallback acotado con presupuesto Always Free (canonica)

> Sustituye a docs/NO_LLM_POLICY.md (decision del dueno, 2026-10-08: el
> auto-repair completo vive en este repo y el LLM deja de estar prohibido).
> La NN sigue siendo la via principal; el LLM es un fallback de PROPUESTA
> que se reemplaza solo con el tiempo (distilacion).

## Quien hace que

| Capa | Componente | Que hace |
|------|-----------|----------|
| Decision principal | `repair_nn_core` (WASM) | Clasifica -> `operator_id`, params, confidence/risk |
| Propuesta fallback | `worker/src/worker/llm_fallback.rs` | LLM (Workers AI) propone UNA accion estructurada JSON |
| Validacion | `repair_operators::gate` (0.55/0.45) | La salida del LLM pasa por el MISMO gate: no hay camino especial |
| Ejecucion | `repair_operators` (deterministas) | Unico ejecutor de acciones tipadas; el LLM NUNCA escribe codigo libre |
| Autoridad | GitHub Actions (VERIFY) | Unico PASS/FAIL real; el worker nunca se autoaprueba |
| Reemplazo | ledger (replay) + promote-model (distilacion) | La NN aprende de los casos origin=llm |

## Regla de oro

El LLM solo propone `{operator, parameters, confidence, risk}` (ids 1
DEPENDENCY_REPAIR, 9 VERSION_PIN, 2 SYNTAX_FIX y 8 IMPORT_PATH_FIX). Los
operadores 2 y 8 son UNA edicion acotada de sustitucion unica y exigen los
parametros `file` (ruta copiada del diagnostico), `from` (texto exacto
presente en el archivo) y `to` (texto de reemplazo): edicion minima, sin
codigo libre. La localizacion determinista (`file`/`from` extraibles del
diagnostico) la aporta param_derive.rs y tambien rellena la propuesta del
LLM; `to` es lo unico que el LLM aporta y jamas se deriva sin el. Cualquier
salida que no pase el gate, el self_guard o el circuit breaker =>
`llm_gate_denied` / blocked. El LLM jamas ve el arbol del repo y jamas
produce diffs.

## Always Free: presupuesto y fail-closed

- Workers AI incluye 10.000 Neurons/dia en el plan Free (renueva 00:00 UTC).
- `llm_fallback.rs` mantiene el contador `llm/budget` (fecha UTC + gastado)
  en el KV AGENT_CONFIG y descuenta `LLM_COST_PER_CALL` (default 300) por
  llamada contra `LLM_DAILY_BUDGET` (default 8000, margen sobre las 10.000).
- Presupuesto agotado, KV caido, binding ausente o salida invalida => None y
  el consumidor escala a humano (fail-closed). La request 10.001 nunca
  falla sola: la puerta se cierra ANTES.
- `LLM_ENABLED = "false"` desactiva el fallback sin redeploy de codigo
  (config en caliente).

## Distilacion: el LLM se reemplaza solo

1. La NN rechaza (gate) -> el LLM propone -> el PR pasa VERIFY (PASS).
2. El caso queda en REPAIR_CASES_KV con `origin=llm` y reward real tras el
   callback; el ledger registra la firma para replay inmediato (la proxima
   ocurrencia de esa firma ya no consulta al LLM).
3. El entrenamiento offline (promote-model) aprende de los casos origin=llm
   igual que de los origin=nn; el challenger se evalua y se promueve.
4. Con el tiempo la NN resuelve esas firmas con confianza >= 0.55 y el LLM
   no se consulta. El presupuesto diario solo se gasta en casos NUEVOS.

## Modelo

Default `LLM_MODEL = "@cf/meta/llama-3.2-3b-instruct"` (pequeno, barato en
Neurons, elegible en el free tier). Verificar contra el catalogo vigente
antes de cambiarlo: los modelos grandes (GLM-5.x, Kimi, DeepSeek-Pro)
requieren plan pagado desde 2026-07. Un modelo retirado devuelve error y el
fallback queda fail-closed (escalacion), nunca fail-open.

## Cadena de autoridad (inviolable)

CHANGE -> POLICY (gate + self_guard + circuit) -> PATCH VALIDATION
-> CI -> VERIFY -> PUSH AUTHORIZATION -> main (ref efimera de PR se borra).

Una sola rama persistente: main.
