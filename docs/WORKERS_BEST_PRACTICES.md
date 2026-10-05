# Mejoras del Worker — Referencias externas y backlog (2026-10-05)

Auditoría del Worker (workers-rs) contra repos externos de referencia y
documentación oficial de Cloudflare. El Worker ya cumple bien los
fundamentos (DO por repositorio, Queue + DLQ, fail-closed, idempotencia,
anti-loop, observability). Este documento registra mejoras candidatas con
su fuente externa.

## Referencias externas consultadas

- cloudflare/workers-rs (framework; patrones de router y bindings)
- cloudflare/rustwasm-worker-template (estructura de build WASM)
- deathbyknowledge/ripgit (uso avanzado de Durable Objects en Rust)
- Documentación oficial: Rules of Durable Objects, Limits, Rate Limiting
  binding, Storage options.

## Backlog priorizado

### P1 — Native Rate Limiting binding para /webhook
Hoy la cuota vive en el DO (serializa TODO el tráfico de webhook por un
solo objeto por repo). El binding nativo de Cloudflare Rate Limiting
filtra ráfagas en el edge ANTES de tocar el DO. Cambio en wrangler.toml:
```toml
[unsafe.binds]
{ name = "WEBHOOK_RATE_LIMITER", type = "ratelimit" }
# rate_limiter: { algorithm = "fixed_window", interval = 60, requests = 30 }
```
Primero confirmar soporte en workers-rs actual; si no existe binding de
rate limiter en la versión del crate, mantener la cuota del DO y reevaluar.

### P2 — Alarms del DO en vez de cron horario para retención
El monitor barre cada hora aunque no haya actividad (invocaciones gratis,
pero ruido en logs). Un DO alarm (setAlarm) dispara la retención solo
cuando hay incidentes vivos, cumpliendo "design around your atom of
coordination".

### P3 — Observability con head_sampling_rate
`[observability]` ya está activo; añadir `head_sampling_rate = 1`
explícito (o <1 si el volumen sube) evita sorpresas de facturación de logs.

### P4 — Smoke test del Worker en CI (cerrar el UNKNOWN)
consistency-verify reporta `SMOKE_WORKER_URL: UNKNOWN` en cada corrida.
Configurar `WORKER_URL` (staging) como secret del repo hace el chequeo
determinista y elimina la única incógnita del reporte.

### P5 — R2 para artefactos grandes
Si los RepairCases crecen, KV no es el lugar (1 write/s por clave).
Registrar payloads grandes en R2 y dejar en KV solo el índice.

### P6 — Limpieza de issues de diagnóstico
Los issues #38–#52 son logs automatizados (diag-verify/diag-cleanup) que
ya no aportan: el snapshot 441705f está OVERALL: PASS. Cerrarlos con un
comentario de archivo y mantener solo los abiertos recientes.

## No-hacer (decisión explícita)

- NO migrar a D1: el estado transaccional por repo encaja en DO SQLite.
- NO subir max_concurrency de la cola: 1 es intencional (orden estricto
  por incidente y presupuesto de reparación).
- NO añadir rutas LLM: ver docs/NO_LLM_POLICY.md.
