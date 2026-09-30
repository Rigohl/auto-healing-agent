# DISCREPANCIES — FASE 1 (Prompt Pad)

**Fecha:** 2026-09-30 · **Repo:** `Rigohl/auto-healing-agent` · **Rama:** `main`

## Arquitectura real

| Pieza | Path | Notas |
|-------|------|-------|
| Gateway legacy | `worker.js` | JS → repository_dispatch |
| Agente legacy | `agent.ts` | LLM HF escribe files |
| CI legacy | `.github/workflows/auto-repair.yml` | Corre agent.ts |
| Tipos NN | `crates/repair_types` | V0 64-dim en main |
| Encoder | `crates/feature_engine` | extract 64 |
| NN | `crates/repair_nn_core` | 64→32→16 + heads |
| WASM | `crates/repair_nn_wasm` | wasm-bindgen |
| Operadores | `crates/repair_operators` | stubs + gate |
| Worker Rust | `worker/` | skeleton; no desplegado |

## vs Prompt Pad

| Requisito | Estado |
|-----------|--------|
| NN no genera código | OK en crates; legacy agent.ts aún existe |
| Features 64 | OK en main |
| AST + CFG + DFG | **Falta** (Fase 2 profunda) |
| Operadores AST reales | **Falta** (stubs) |
| Worker desplegado + KV/R2 | **Falta** |
| tests/ fixtures | **Falta** directorio tests/ |
| scripts train/export | **Falta** |
| DISCREPANCIES | Este archivo |
| Pony | **Excluido** deliberadamente |
| Solo rama main | Política en BRANCH_POLICY |

## Limitaciones de herramientas

- Mem0: no operativo — no consultado
- Elicit: sin API — no usado
- Deploy CF: no ejecutado en esta sesión

## Siguiente

1. `tests/fixtures` + CI verde en cargo test
2. Operadores AST mínimos (p.ej. package.json)
3. Cablear Worker → features → NN (sin HF)
