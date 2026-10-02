# Auto-Healing Agent — Índice de Documentación

> Repo canónico: `Rigohl/auto-healing-agent` · Una sola rama persistente: `main`

## Navegación rápida

| Doc | Contenido |
|-----|-----------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Flujo completo Incident → NN → Gate → PR → VERIFY |
| [PROMPT_PAD.md](PROMPT_PAD.md) | Contrato de implementación (fases, reglas) |
| [DISCREPANCIES.md](DISCREPANCIES.md) | Decisiones repo vs Notion |
| [INVENTORY.md](INVENTORY.md) | Estado real del árbol |
| [GOVERNANCE.md](GOVERNANCE.md) | Umbrales 0.55 / 0.45, flags y matriz de transiciones |
| [CONTRACT.md](CONTRACT.md) | Contrato GitHub ↔ Cloudflare v1: eventos, idempotencia, matriz de errores |
| [BRANCH_POLICY.md](BRANCH_POLICY.md) | Por qué solo `main` persiste y qué se hizo con cada rama |
| [NO_LLM_POLICY.md](NO_LLM_POLICY.md) | Por qué no hay LLM ni fallback híbrido |
| [E2E_CHECKLIST.md](E2E_CHECKLIST.md) | Checklist FASE 9 + qué está verde hoy |
| [REFERENCES.md](REFERENCES.md) | Enlaces oficiales + glosario del código |
| [PART1_REPOSITORY.md](PART1_REPOSITORY.md) | Estructura y crates |
| [PART2_NEURAL_NETWORK.md](PART2_NEURAL_NETWORK.md) | NN como clasificador, 64→32→16 |
| [PART3_CLOUDFLARE_RUNTIME.md](PART3_CLOUDFLARE_RUNTIME.md) | Worker orquestador, límites Free |
| [PART4_PERSISTENCE_TRANSVERSAL.md](PART4_PERSISTENCE_TRANSVERSAL.md) | MongoDB / Notion / Mem0 / regla Context7 |
| [MEM0_STATUS.md](MEM0_STATUS.md) | Estado real del conector + diseño futuro |

## Principios
1. Worker = orquestador (no motor de cómputo).
2. Inferencia edge / entrenamiento offline.
3. $0 en Free Tier (10 ms CPU, 128 MB, 64 MiB bundle).
4. Memoria separada por responsabilidad.
5. Regla Context7: verificar API antes de escribir código.

## Estado resumido
- crates: types (+ `contract`) / feature_engine (V1) / nn_core / nn_wasm / operators → ✅
- worker → runtime PART3async: webhook fail-closed → DO (dedup, quota, anti-loop) → Queue → 202
- model/ → placeholders (`weights: null`; no hay entrenamiento todavia)
- docs/verification_evidence.json → artefacto de CI, no versionado
- legacy/ → archivado (no ejecutar)
- Mem0 → pendiente de conector

## Comprobaciones ejecutables

| Script | Qué guarantee |
|---------|---------------|
| `scripts/verify_repo.py` | Deriva entre docs, codigo, workflows y refs de git (45 claims) |
| `scripts/validate-preflight.sh` | Logica de Preflight del deploy y permisos de los scripts de build |
