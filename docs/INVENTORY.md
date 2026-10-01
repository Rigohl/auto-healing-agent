# INVENTORY — post-unificación (2026-09-30)

Fuente: main + rama `chore/unify-docs-cleanup`.

## Árbol relevante (docs)

```
docs/
├── ARCHITECTURE.md
├── DISCREPANCIES.md      ← actualizado (ítems 16-17 unificación)
├── INVENTORY.md          ← este archivo
├── PROMPT_PAD.md
├── GOVERNANCE.md
├── PART1_REPOSITORY.md   ← nuevo (conciso)
├── PART2_NEURAL_NETWORK.md ← nuevo
├── PART3_CLOUDFLARE_RUNTIME.md ← nuevo
├── PART4_PERSISTENCE_TRANSVERSAL.md ← nuevo
├── AGENTS.md, CODEMAP.md, INDEX.md, PHASE_STATUS.md, …
└── …
```

## Ramas (estado objetivo)

- **main** — única rama persistente.
- chore/unify-docs-cleanup — efímera (este PR).
- feature/*, docs/*, setup/*, kilo/* — a eliminar tras merge.

## Código (sin cambios en esta unificación)

- crates/ 5 miembros (types, feature_engine, nn_core, nn_wasm, operators)
- worker/ skeleton
- model/ placeholders
- legacy/ archivado
- FeatureVector DIM=64, WEIGHT_COUNT=2863, gate 0.55/0.45

## Decisiones clave

Repo = fuente de verdad del código.  
Notion = diseño/histórico (actualizar sección estado).  
PART*.md = diseño consolidado y conciso.  
Una sola rama persistente: main.
