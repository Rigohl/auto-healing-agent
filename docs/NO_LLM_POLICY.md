# Política de decisión — núcleo sin LLM (canónica)

> Absorbida de `feat/rust-nn-core:docs/HYBRID_POLICY.md` (commit `a886669`).
> Renombrada: el documento nunca propuso una arquitectura híbrida, la
> **descartó**. Ver `docs/DISCREPANCIES.md` ítems 3 y 13.

## Aclaración sobre "fallback"

En mensajes anteriores se usó "LLM fallback" para **no romper** el `agent.ts`
actual mientras migrábamos. **Eso NO es la arquitectura objetivo.**

Según el contrato canónico (SYSTEM PROMPT — Agent Health):

> El modelo neuronal (MLP) **SOLO clasifica y propone**
> (`operator_id` + params + confidence/risk).
> **NUNCA genera código arbitrario**, NUNCA escribe contenido completo de archivos.

El texto de diseño que seguimos es explícito:

> núcleo de Agent Health = **Rust + WASM + red neuronal pequeña, sin LLM**.

## Quién hace qué

| Capa | Component | Qué hace |
|------|-----------|----------|
| Decisión | `repair_nn_core` (WASM) | Clasifica → `operator_id`, confidence, risk |
| Ejecución | `repair_operators` | Transformación **determinista** acotada |
| Autoridad | Policy → CI → VERIFY | Allow/deny + PASS/FAIL |
| Legacy V0 | eliminado del repo el 2026-10-05 | Era `legacy/agent.ts` (HF), archivo V0; ruta borrada por decisión del dueño |

Si confidence/risk no pasan el gate → `blocked_by_policy` o `needs_human`,
**no** invocar generación libre de código.

## Agentes lógicos

Roles del pipeline, **no** procesos LLM. Implementados como fases de
`PipelinePhase` en `crates/repair_types/src/lib.rs`:

1. **Detector** — webhook / incidente
2. **Evidence** — logs, stack, firma
3. **Diagnostic** — FailureSignature + features (no ejecuta)
4. **Neural Policy** — MLP → RepairAction
5. **Policy Engine** — autoriza o bloquea
6. **Repair Operator** — único ejecutor de acciones tipadas
7. **Verify** — CI / evidencia real
8. **Rollback / Escalation** — FAIL → rollback o humano

## Cadena de autoridad (inviolable)

```
CHANGE → POLICY → PATCH VALIDATION → CI → VERIFY
      → PUSH AUTHORIZATION → main (ref efímera de PR se borra)
```

Una sola rama persistente: **main**.

## Defaults

| Knob | Default |
|------|---------|
| `MIN_CONFIDENCE` | 0.55 |
| `MAX_RISK` | 0.45 |
| `AUTO_MERGE` | false |
| `PRODUCTION_WRITE` | false |
| Operadores de secretos/env | advisory / block |

Umbrales: `docs/GOVERNANCE.md` es la fuente de verdad de 0.55 / 0.45
(`DISCREPANCIES` ítems 11 y 18).