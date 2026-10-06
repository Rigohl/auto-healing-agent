# Stack Cloudflare + Agent Auto-Repair — Análisis y decisión

> Basado en el análisis "Cloudflare + Agent Auto-Repair" (PDF del propietario, octubre 2026).
> Verificación en vivo de los 11 repos: 2026-10-06 (existencia, actividad y compatibilidad).
> Autoridad de filtro: docs/NO_LLM_POLICY.md (sin LLM en el path de producción),
> docs/AGENTS.md y la regla "Cloudflare ORCHESTRATES... GitHub Actions es VERIFY".

## Resumen ejecutivo

El PDF propone 3 planos (control/repair, aplicación/edge, networking). Tras
verificar los 11 repos contra ESTE proyecto:

1. **La pieza central del plano de aplicación YA ESTÁ HECHA en `main`**:
   el worker usa workers-rs 0.8 con la feature `axum` + `axum = "0.8"`
   (patrón del ejemplo oficial `examples/axum` de workers-rs), migrado
   desde el Router legacy conservando cuerpos y códigos de respuesta
   exactos (dependen de ellos el smoke test de deploy.yml y los claims de
   scripts/verify_repo.py). Ver `worker/Cargo.toml` y `worker/src/lib.rs`.
2. **`axum-cloudflare-adapter` NO se necesita**: es el enfoque de la era
   pre-http de workers-rs ("keep the http flag disabled"); workers-rs 0.8
   ya trae soporte Axum nativo, que es lo que usa este repo. Usarlo sería
   un paso atrás.
3. **Los orquestadores LLM (zeroshot, SoloDawn, amux) NO entran al path de
   producción** por NO_LLM_POLICY.md. Se adoptan sus PATRONES, no sus
   dependencias — y buena parte ya está implementada aquí de forma
   determinista.

## Verificación y decisión por repo (2026-10-06)

### Plano de control / repair

| Repo | Verificado | Decisión para este proyecto |
|------|-----------|------------------------------|
| [the-open-engine/zeroshot](https://github.com/the-open-engine/zeroshot) | ✅ real, activo (~1.9k) | **PATRÓN, NO DEPENDENCIA**. Su ciclo implement→review→repair→gate con grafo explícito acotado y ledger SQLite durable es exactamente la filosofía de este repo (pipeline determinista + DO como ledger). La diferencia: aquí el grafo es fijo y sin agentes LLM. Ya cubierto por: feature_engine→NN→gate→diff acotado→PR→VERIFY (Actions). |
| [huanchong-99/SoloDawn](https://github.com/huanchong-99/SoloDawn) | ✅ real, activo | **RECHAZADO en producción** (orquesta Claude Code/Gemini/Codex = LLM). Su idea de "quality gates anti-alucinación antes de considerar un diff listo" ya tiene equivalente aquí: edits acotados deterministas (`diff.rs`: sin codegen libre, major-bump rechazado) + VERIFY autoritativo en Actions. |
| [mixpeek/amux](https://github.com/mixpeek/amux) | ✅ real, activo | **OPCIONAL/FUTURO**: control plane Rust de agentes con self-healing recovery. Solo aplicaría para un daemon de desarrollo local (fuera del worker), nunca en el path edge. No hay caso de uso hoy. |

### Plano de aplicación / edge

| Repo | Verificado | Decisión |
|------|-----------|-----------|
| [cloudflare/workers-rs](https://github.com/cloudflare/workers-rs) | ✅ oficial | **YA ES LA BASE** (`worker = "0.8"`, features queue+http+axum). |
| [logankeenan/axum-cloudflare-adapter](https://github.com/logankeenan/axum-cloudflare-adapter) | ✅ real (v0.14.0, ene-2025, 54k descargas) | **NO ADOPTAR**: diseñado para workers-rs SIN feature http; este repo ya usa la vía oficial (worker 0.8 "axum"). Mantenerlo como referencia histórica. |
| [cloudflare/rustwasm-worker-template](https://github.com/cloudflare/rustwasm-worker-template) | ✅ oficial | **REFERENCIA**: su configuración de perfil release (lto, codegen-units=1) ya está replicada en worker/Cargo.toml (opt-level="s" + fix de --panic-unwind: strip ELIMINADO, PR #33). |

### Plano de datos / networking

| Repo | Verificado | Decisión |
|------|-----------|-----------|
| [cloudflare/pingora](https://github.com/cloudflare/pingora) | ✅ real (~27.6k) | **FUERA DE SCOPE del worker** (binario Rust standalone, no WASM). Aplicaría solo en la arquitectura self-hosted (B del PDF) como proxy delante de open-compute. El edge de Cloudflare ya ES el plano de datos en la arquitectura A. |
| [cloudflare/quiche](https://github.com/cloudflare/quiche) | ✅ real | **N/A**: HTTP/3 lo termina la red de Cloudflare delante del Worker; el worker no hace QUIC. |
| [cloudflare/wirefilter](https://github.com/cloudflare/wirefilter) | ✅ real | **PATRÓN YA ADOPADO**: el motor de expresiones declarativas de REPAIR_RULES (worker/src/worker/rules.rs, P1, fail-closed) sigue la misma idea de filtros programables. Como dependencia, N/A. |
| [cloudflare/boringtun](https://github.com/cloudflare/boringtun) | ✅ real | **N/A** para este proyecto (WireGuard/Zero Trust es infra del usuario, no del worker). |
| [elliothux/open-compute](https://github.com/elliothux/open-compute) | ✅ real | **ALTERNATIVA DE PORTABILIDAD (B)**: runtime self-hosted compatible Workers (KV/D1/R2/DO/Queues) en un binario Rust. Registrar como plan de contingencia si se necesita salir de Cloudflare; el repo actual es portable casi sin cambios (wrangler.toml → config de open-compute). |
| [CluvexStudio/Aether](https://github.com/CluvexStudio/Aether) | ✅ real | **N/A** (core WARP/MASQUE para redes censuradas). Sin caso de uso. |

## Mapa PDF → estado real del proyecto

| Propuesta del PDF | Estado aquí (2026-10-06) |
|---|---|
| Axum sobre workers-rs como plano de control edge | ✅ **Hecho**: worker 0.8 feature "axum" + axum 0.8 (migración completada en main; smoke test y verify_repo.py conservan contratos) |
| Ciclo implement→review→repair→gate | ✅ **Equivalente determinista**: webhook→DO→queue→NN→gate(0.55/0.45)→diff acotado→PR→Actions VERIFY→callback→DO |
| Anti-alucinación antes de "listo" | ✅ **Por construcción**: edits acotados from→to, sin codegen libre (diff.rs); VERIFY autoritativo en CI |
| Ledger durable del ciclo | ✅ Durable Object SQLite (IncidentState) |
| RepairCase persistente | ✅ REPAIR_CASES_KV (namespace real 996211a0...) |
| Proxy/gateway (pingora) | N/A en edge-first (arquitectura A); plan B = open-compute |

## Conclusión

**Prioridad WASM/Rust confirmada y ya satisfecha en el plano de aplicación**:
el worker es 100% Rust→WASM (workers-rs 0.8 + axum 0.8 + crates no_std).
Los repos del PDF que aportan algo nuevo (pingora, open-compute) pertenecen a
la arquitectura self-hosted (B), no al worker edge (A). Las propuestas LLM
(zeroshot/SoloDawn/amux) quedan fuera del path de producción por diseño
(NO_LLM_POLICY.md); sus patrones ya tienen equivalente determinista aquí.

Pendientes siguen siendo los HUMANOS: deploy real del worker (placeholder
vigente) + secrets GITHUB_TOKEN/WEBHOOK_SECRET (Linear PYH-61/PYH-62),
y luego E2E real (PYH-63).
