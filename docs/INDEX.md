# Auto-Healing Agent — Índice de Documentación

> Repo canónico: `Rigohl/auto-healing-agent` · Una sola rama persistente: `main`

## Navegación rápida

| Doc | Contenido |
|-----|-----------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Flujo completo Incident → NN → Gate → PR → VERIFY |
| [PROMPT_PAD.md](PROMPT_PAD.md) | Contrato de implementación (fases, reglas) |
| [DISCREPANCIES.md](DISCREPANCIES.md) | Decisiones repo vs Notion |
| [INVENTORY.md](INVENTORY.md) | Estado real del árbol |
| [GOVERNANCE.md](GOVERNANCE.md) | Umbrales 0.55 / 0.45 + flags |
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
- crates: types / feature_engine / nn_core / nn_wasm / operators → ✅
- worker → esqueleto
- model/ → placeholders
- legacy/ → archivado (no ejecutar)
- Mem0 → pendiente de conector
