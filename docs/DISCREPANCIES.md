# DISCREPANCIES — FASE 1 + Unificación (2026-09-30)

Regla: cada ítem tiene decisión. Sin decisión documentada → bloqueante.

| # | Elemento | Notion | Repo (`main`) | Decisión | Justificación |
|---|----------|--------|--------------|---------|-------------|
| 1 | Dims del FeatureVector | Snippet 16; estado 16→32→11 (907 pesos) | **64**, schema v2, 2617 pesos | **Repo gana: 64-dim** | PROMPT_PAD regla 2. Snippet 16 superado. |
| 2 | package.json / Node runtime | Implícito | **No existe** | **Rust-only** | legacy/agent.ts es archivo muerto. |
| 3 | Path LLM (agent.ts + HF) | Política híbrida + HYBRID_POLICY.md | legacy/ + auto-repair disabled; HYBRID_POLICY no existe | **Rust-only. Híbrido descartado** | No reintroducir. |
| 4 | tests/ directorio | Propuesto | Tests inline en crates | **Mantener inline** | Evitar ruido. |
| 5 | Scripts train/export | Propuestos | Solo set-github-secrets.sh | **Diferir** hasta pesos reales. |
| 6 | model/README.md | Propuesto | 404 | Crear en FASE 5. |
| 7 | LICENSE / Cargo.lock | Propuestos | 404 | Añadir Cargo.lock; LICENSE MIT declarado. |
| 8 | Worker cableado a NN | Diseñado | Skeleton (health/KV/webhook) | Cablear en FASE 6. |
| 9 | Mem0 / R2 / D1 / DO | Diseñado | Solo KV placeholder | KV primero; resto diferido. |
| 10 | MongoDB en Rust core | Persistencia | Sin driver | Diferido; MongoDB sigue SoT histórico. |
| 11 | Umbrales conf/risk | Sin números explícitos | 0.55 / 0.45 | Repo gana. |
| 12 | Operadores | 5 outputs | OperatorId 0..12 (13) | Repo gana. |
| 13 | should_fallback_to_llm | Mencionado | No existe | No reintroducir. |
| 14 | Páginas Notion duplicadas | Dos páginas | N/A | Conservar canónica 3eb50567-40be-8146-b75a-e90f6f818580. |
| 15 | Estado implementación Notion | 16-dim / híbrido | 64-dim / rust-only | Actualizar Notion. |
| 16 | Ramas feature/docs/setup/kilo | N/A | Múltiples ramas | **Unificar en main**. Extraer PART*.md + docs útiles; borrar ramas residuales. Una sola rama persistente = main (SYSTEM PROMPT). |
| 17 | PART1–4 design docs | PDF / ramas | Ahora en docs/PART*.md (rama cleanup) | Añadidos de forma concisa. |

## Gate FASE 1 + Unificación

- [x] Discrepancias 1–17 con decisión.
- [x] PART1–4 añadidos (concisos).
- [ ] PR cleanup mergeado y ramas residuales eliminadas.

**Siguiente**: merge chore/unify-docs-cleanup → main → borrar ramas feature/*, docs/*, setup/*, kilo/*.
