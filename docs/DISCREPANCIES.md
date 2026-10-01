# DISCREPANCIES — FASE 1 + Unificación + Enrichment (2026-09-30)

Regla: cada ítem tiene decisión. Sin decisión documentada → bloqueante.

| # | Elemento | Notion / otras ramas | Repo (`main`) | Decisión | Justificación |
|---|----------|----------------------|--------------|---------|-------------|
| 1 | Dims FeatureVector | 16 / 907 pesos | **64 / 2617** | Repo gana | PROMPT_PAD regla 2 |
| 2 | Node / package.json | Implícito | No existe | Rust-only | legacy archivado |
| 3 | Path LLM / híbrido | HYBRID_POLICY | No existe | Descartado | Rust-only |
| 4 | tests/ dir | Propuesto | Inline | Mantener inline | |
| 5 | Scripts train/export | Propuestos | Diferir | Hasta pesos reales |
| 6 | model/README | Propuesto | Diferir | FASE 5 |
| 7 | LICENSE / Cargo.lock | Propuestos | Añadir lock | |
| 8 | Worker → NN | Diseñado | Skeleton | FASE 6 |
| 9 | Mem0/R2/D1/DO | Diseñado | Solo KV | Diferido |
| 10 | MongoDB Rust | Diseñado | Sin driver | Diferido |
| 11 | Umbrales conf/risk | (sin nº) | **0.55 / 0.45** | Repo gana | GOVERNANCE.md |
| 12 | Operadores | 5 | 13 (0–12) | Repo gana | |
| 13 | should_fallback_to_llm | Mencionado | No | No reintroducir |
| 14 | Notion duplicadas | 2 páginas | Conservar canónica | |
| 15 | Estado Notion | 16-dim/híbrido | Actualizar Notion | |
| 16 | Ramas residuales | Varias | Unificar → main | Borrar tras merge |
| 17 | PART1–4 | PDF / ramas | docs/PART*.md | Añadidos concisos |
| 18 | Gate thresholds | kilo: 0.80 / 0.25 | **0.55 / 0.45** | **Repo gana (0.55/0.45)** | GOVERNANCE.md es SoT. El diseño kilo es propuesta futura/más estricta; no se cambia sin decisión explícita. |
| 19 | Docs ricos kilo | 00_INDEX, MEM0_STATUS, gate detallado | INDEX + MEM0_STATUS + GOVERNANCE enriquecido | Absorbido lo esencial | Sin bloat |

## Estado
- [x] PART1–4 + INDEX + MEM0_STATUS + GOVERNANCE enriquecido
- [ ] Merge enrichment PR
- [ ] Borrar todas las ramas residuales (solo main)
