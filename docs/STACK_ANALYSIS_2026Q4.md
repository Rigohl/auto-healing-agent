# Análisis del stack "Cloudflare + Agent Auto-Repair" (Octubre 2026)

Contraste a fondo entre el stack definitivo definido en el documento externo
(zeroshot + SoloDawn / Axum sobre workers-rs / pingora + quiche + wirefilter +
boringtun) y la arquitectura real de este repo, con veredicto por plano y
regla de adopción compatible con docs/NO_LLM_POLICY.md.

## Plano de Control (zeroshot, SoloDawn, amux)

**Código real analizado:**

- **zeroshot** (the-open-engine): orquestador implement→review→repair→gate.
  Los revisores son agentes LLM (Codex/Claude/Copilot). Su tesis central:
  "el agente que escribió el código no debe ser el que dice que funciona".
- **SoloDawn**: orquestador de claude-code/codex con **31 reglas de calidad +
  3 gates + self-healing loop + barra de aceptación de 90 puntos**. Su propio
  README es honesto: "no puede impedir la alucinación en el momento de
  emitir tokens; atrapa los artefactos antes de entregar".
- **amux**: control plane Rust para agentes de coding con recovery.

**Contraste con este repo:** nuestro equivalente del flujo
implement→review→repair→gate ya existe en forma determinista:
operador (repair_operators) → gate (MIN_CONFIDENCE/MAX_RISK + rules.rs) →
GitHub Actions VERIFY (autoridad) → RepairCase. No usamos LLM en producción
(NO_LLM_POLICY), y por tanto zeroshot/SoloDawn **no son dependencias del
worker**: encajan como capa de orquestación de DESARROLLO (fuera del runtime,
p. ej. lado GitHub Actions / máquina del dev), exactamente como el propio
documento los ubica ("zeroshot/SoloDawn fuera o en Containers").

**Qué adoptamos:** el patrón "independent review + repair loop + gates"
(ya es el P4 del roadmap: VERIFY=FAIL re-alimenta al feature_engine→NN,
máx. 3 intentos, escalación a humano). Las "31 reglas + gates" de SoloDawn
valen como referencia para ampliar rules.rs (hoy solo block/observe).

## Plano de Aplicación (axum-cloudflare-adapter, workers-rs, template)

**Verificado en código:** workers-rs expone hoy un ejemplo oficial
`examples/axum` (`#[event(fetch)]` con HttpRequest + tower Service::call),
más `examples/tracing` (tracing_subscriber JSON + tracing_web, compatible
wasm32). El adapter de logankeenan es la vía clásica (feature `http`).

**Contraste con este repo:** `worker/src/lib.rs` usa el `worker::Router`
legacy (`.get_async`/`.post_async` + RouteContext). Funcional pero es la
vía congelada; el ejemplo oficial confirma que Axum con with_state +
extractors tipados + middleware tower es el camino soportado. Regla del
documento externo: "las APIs HTTP nuevas deben exponerse vía Axum".

**Qué adoptamos (PR propuesto, prioridad alta):**
1. Migrar /webhook, /github/callback, /model, /health a Router de Axum; el
   secret fail-closed pasa a middleware::from_fn (testable aislado).
2. tracing JSON estructurado (ejemplo oficial) — habilita las métricas de
   reparación (operator_id, confidence, risk, verify_result) del roadmap P2
   y alimenta Workers Analytics Engine.
3. DO incident_state: el match de rutas del fetch interno también puede
   moverse a Axum cuando el adapter lo soporte en DO (validar en rama).

## Plano de Datos (pingora, quiche, wirefilter, boringtun, open-compute)

**Veredicto directo:** NINGUNO compila a wasm32-unknown-unknown. pingora y
quiche son servicios Tokio nativos; wirefilter requiere motores de firewall
nativos; boringtun es userspace de red. Para este worker son
**inaplicables como dependencias** — solo tienen sentido en una
arquitectura B self-hosted (open-compute como runtime compatible,
pingora delante como proxy), que hoy no es nuestro objetivo.

**open-compute** (1.5k★, Rust puro) sí es relevante como **backend de
staging propio**: permitiría ejecutar el smoke test end-to-end
(SMOKE_WORKER_URL) sin depender del tier gratis de Cloudflare.

## Mapping resumido

| Componente PDF | Estatus en este repo | Acción |
|---|---|---|
| zeroshot (repair loop) | Equivalente determinista (gate+VERIFY) | Adoptar patrón en P4 (retry NN) |
| SoloDawn (reglas/gates) | rules.rs + gate | Ampliar rules (referencia) |
| amux | No aplica (sin daemon) | — |
| axum-cloudflare-adapter | worker::Router legacy hoy | **PR de migración a Axum** |
| workers-rs | En uso 0.8.x | Actualizar al usar feature http/tracing |
| rustwasm-worker-template | Template base, ya superado | — |
| pingora/quiche/wirefilter/boringtun | No compilables a WASM | Solo arquitectura B self-hosted |
| open-compute | — | Opcional: staging para SMOKE_WORKER_URL |

## Decisión

El stack del PDF es válido para una **plataforma** de agentes LLM; este repo
es un **worker determinista** con política sin-LLM. La adopción correcta es
por patrones, no por dependencias: Axum en el plano de aplicación (única
incorporación técnica real), el loop de repair de zeroshot como P4 del
roadmap, y las reglas de calidad de SoloDawn como guía de rules.rs.
