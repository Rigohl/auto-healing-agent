# PART 4 – Persistencia, Documentación y Regla Transversal

> MongoDB = memoria operacional real.
> Notion = diseño arquitectónico (histórico, **no** autoridad del código).
> Mem0 = memoria semántica (preparada; conector aún no operativo).

## MongoDB Atlas (Cluster0 / auto_healing_agent)
Colecciones: audit, incidents, postmortems, repair_rules, training_examples.

Incidente histórico observado: `syntax_error` (vercel-build), 3 intentos, status `repair_failed`.

## Separación de memoria

| Capa | Almacén | Contenido |
|------|---------|-----------|
| Operacional | Durable Objects / KV | state, locks, coordination |
| Semántica | MongoDB (+ Mem0 futuro) | FailureSignature, RepairCase, patterns |
| Aprendizaje | MongoDB | TrainingExample |
| Artefactos | R2 | WASM, checkpoints |
| Punteros | KV | model current/stable |
| Diseño | Notion | arquitectura, runbooks |
| Código | GitHub | source of truth |

## Notion
Página canónica: `auto-healing-agent — Rust/WASM Neural Network Core`.
Autoridad: solo diseño/histórico. Código = GitHub.

## Mem0
Arquitectónicamente preparado. No se finge consulta. Integración pendiente.

## Regla transversal de código (OBLIGATORIA)
Ningún agente escribe código que dependa de API/crate/servicio externo sin verificar documentación actual.

Orden de preferencia:
1. Context7 MCP (resolve-library-id + query-docs)
2. GitHub MCP (workers-rs, wasm-bindgen, burn, tract)
3. Exa / docs.rs / crates.io

Prohibido: usar conocimiento de entrenamiento para firmas de API.

## Fases relacionadas
FASE 1 (inventario vs Notion/Mongo), FASE 5 (TrainingExample), FASE 9 (governance), documentación final.
