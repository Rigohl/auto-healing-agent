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

## Revisión contra documentación de Cloudflare en vivo (2026-10-06)

Nueva pasada contra developers.cloudflare.com (changelog + docs) tras el
bump de actions. Estado: nada roto; tres ítems nuevos al backlog.

### P7 — Actualizar compatibility_date (2024-09-23 -> actual)
El worker usa `compatibility_date = "2024-09-23"` (~2 años). No es un
bug: los cambios de compat date alteran el runtime, así que el bump debe
hacerse deliberado y con CI + smoke test de staging en verde. Cambios
relevantes que se ganarían:
- DO `deleteAll()` borra TAMBIÉN el alarm del objeto (compat >= 2026-02-24);
  hoy la retención P2 debe cancelar alarms a mano al limpiar estado.
- Para compat >= 2026-08-04, `nodejs_compat` viene activado por defecto.
- Límite de 1000 subrequests por invocación ELIMINADO (2026-02-11).
Requisito: confirmar que workers-rs 0.8 soporta la fecha objetivo antes.

### P8 — WebAssembly Exception Handling (futuro, WASM 3.0)
Changelog de Workers: hoy wasm-bindgen trata un panic como estado
inválido del módulo WASM (el runtime reinicializa). El repo ya maneja
panics (hook en lib.rs, sin unwrap() en el path de request). La propuesta
WASM 3.0 permitirá panics recuperables sin reinicialización: reevaluar
cuando Cloudflare lo soporte en producción (no adelantarse: NO_LLM_POLICY
aplica igual, es solo runtime).

### P9 — Rules of Durable Objects (guía oficial 2025-12-15)
Cloudflare publicó una guía opinada de best practices de DO. El diseño
actual ya sigue las reglas principales (un DO por repo como átomo de
coordinación, storage transaccional, alarms). Acción: revisar la guía
contra incident_state.rs en el próximo ciclo de auditoría; no hay
incumplimientos conocidos hoy.

### Ya cubierto (verificado, sin acción)
- Pánico handler por defecto en Rust Workers: el repo ya lo tiene.
- `[observability]` + `head_sampling_rate = 1`: hecho (P3).
- Límite KV de namespaces: ahora 1000 por cuenta (usamos 6, sobra margen).
- Workers Builds: Root directory worker + build.sh + wrangler deploy,
  documentado en wrangler.toml (NO recrear config en raíz).

## No-hacer (decisión explícita)

- NO migrar a D1: el estado transaccional por repo encaja en DO SQLite.
- NO subir max_concurrency de la cola: 1 es intencional (orden estricto
  por incidente y presupuesto de reparación).
- NO añadir rutas LLM: ver docs/NO_LLM_POLICY.md.
