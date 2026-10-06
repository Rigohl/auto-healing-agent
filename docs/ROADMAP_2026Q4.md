# Roadmap Q4 2026 — Auto-Healing Agent

> Generado 2026-10-06 tras análisis del repo (main = 2be2b8b/91f9b23, verify 48/50 PASS) y
> revisión de referencias externas: reglas oficiales de Durable Objects, workers-rs,
> y agentes SRE auto-reparables de referencia (Self-Healing-SRE-Agent y similares).

## Estado actual
- main verde tras reparación integral (PR #56): revert a 18d6ca0 + P2 alarma DO + P3 sampling.
- verify 48/50 PASS; `SMOKE_WORKER_URL` UNKNOWN (falta secret `WORKER_URL`).
- Solo rama `main`; issues de diagnóstico cerrados.

## Prioridades

### P0 — Re-land de param_derive.rs
Issue #64. Reincorporar formateado y con CI verde. Sin esto, la funcionalidad P0 queda incompleta.

### P1 — Secret WORKER_URL (acción humana)
`wrangler` staging URL como secret de GitHub → cierra `SMOKE_WORKER_URL: UNKNOWN` y sube verify a 49-50/50.

### P2 — Observabilidad estructurada: Workers Analytics Engine
En vez de confiar solo en logs muestreados (`head_sampling_rate`), escribir métricas por reparación
(operator_id, confidence, risk, verify_result, latencia) a Analytics Engine. Permite dashboards de
tasa de éxito por operador sin almacenar PII. Referencia: docs de Workers / rustify.rs 2026.

### P3 — Tests de integración con wrangler dev / Miniflare
El repo no tiene tests end-to-end del worker (DO + Queue + KV) en CI. `wrangler dev` (Miniflare)
emula DO/KV/Colas localmente: añadir un workflow que levante el worker y pruebe
/ingest → queue_consumer → PR-gate con fixtures. Esto habría atrapado el truncamiento de 812f3bb.

### P4 — Bucle de reintentos con retroalimentación (patrón Self-Healing-SRE-Agent)
Inspirado en repos de vanguardia: investigador → mecánico → validador con máx. 3 intentos y
retroalimentación del fallo al seleccionador (nuestra NN + feature_engine), con escalación a humano
tras agotar intentos. Hoy el pipeline es single-shot: operador → PR → VERIFY. El feedback de
VERIFY=FAIL debería re-alimentar al feature_engine como nueva entrada (ya no "loop" de código
generado — la NN solo re-selecciona operadores, cumpliendo NO_LLM_POLICY).

### P5 — R2 para artefactos grandes
Si RepairCases crece (>1 KB por caso, miles de casos), mover payloads completos a R2 y dejar
índice ligero en KV. Tier gratis cubre 10 GB.

### P6 — Rate limiting binding nativo
Bloqueado por bugs de cast en workers-rs (issue #700 de cloudflare/workers-rs). Mantener cuota del
DO; reevaluar cuando el binding esté estable.

## Reglas de Durable Objects aplicables (auditoría)
1. El DO INCIDENT_STATE se usa correctamente para coordinación serializada (dedup/idempotencia/quota) — correcto según la guía oficial.
2. P2 (alarma DO para retención) es el patrón recomendado: "scheduled work per entity" via alarms. Ya en main.
3. Mantener gate fail-closed en el Worker ANTES de enrutar al DO: los DO deben usarse para estado, no como proxy de rutas sin estado.

## No hacer (vigente)
- Migrar a D1.
- Subir `max_concurrency` de la Queue (el orden determinista es parte del diseño).
- Rutas LLM en producción (NO_LLM_POLICY).

## Referencias externas
- Cloudflare — Rules of Durable Objects (guía oficial, actualizada 2026-08): https://developers.cloudflare.com/durable-objects/best-practices/rules-of-durable-objects/
- cloudflare/workers-rs: https://github.com/cloudflare/workers-rs
- Self-Healing-SRE-Agent (patrón multi-agente + validador + máx. intentos + aprobación humana): https://github.com/jalpatel11/Self-Healing-SRE-Agent
- Rust on Cloudflare Workers 2026 (Analytics Engine, Miniflare): https://rustify.rs/articles/rust-cloudflare-workers-edge-2026
