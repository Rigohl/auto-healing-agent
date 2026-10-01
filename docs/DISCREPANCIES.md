# DISCREPANCIES — FASE 1 + Unificación + Enrichment (2026-09-30)

Regla: cada ítem tiene decisión. Sin decisión documentada → bloqueante.

| # | Elemento | Notion / otras ramas | Repo (`main`) | Decisión | Justificación |
|---|----------|----------------------|--------------|---------|-------------|
| 1 | Dims FeatureVector | 16 / 907 pesos | **64 / 2863** | Repo gana | PROMPT_PAD regla 2 |
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
| 20 | WEIGHT_COUNT | Docs del repo decían 2617 | **2863** (código) | **Repo código gana** | `crates/repair_nn_core/src/lib.rs:18`: `64*32+32 + 32*16+16 + 16*13+13 + 16+1 + 16+1` = 2863. El 2617 era una cifra heredada de una variante de arquitectura anterior y no correspondía a ningún código. Corregido en ARCHITECTURE.md, PART2, INVENTORY.md y este ítem 1. |
| 21 | MongoDB "✅ Conectado" | `kilo/bionic-owl-ok9:docs/DOCUMENTATION.md` afirma *"MongoDB Atlas ✅ Conectado (Cluster0)"* | Sin driver Rust en código | **Rechazado** | Constraint: no declarar_SUCCESS ni inventar conectores. MongoDB sigue diferido (ítem 10). El resto de ese doc de 491 líneas se descartó por este motivo. |
| 22 | Nº de agentes lógicos | `AGENTS.md` = 14 · `HYBRID_POLICY.md` = 8 | Código: `PipelinePhase` tiene 15 variantes | **Grano de diseño, no de código** | Sin contradicción funcional: los agentes son fases del pipeline y el código sólo codifica fases. `NO_LLM_POLICY.md` conserva la lista de 8 y señala `AGENTS.md` como desglose fino de 14. No bloquea. |
| 23 | Pony / actor runtime | `feat/rust-nn-core:docs/PONY.md` (FASE 8) | `docs/AGENTS.md` ya dice *"Pony / actor runtime nativo: fuera de este repo"* | **Descartado** | El propio AGENTS.md lo excluye. PONY.md no se absorbe; su decisión ("not started by design order") ya está registrada allí. |
| 24 | Referencias académicas kilo | `ACADEMIC_REFS.md` cita 9 papers con títulos, autores y hallazgos | **No verificable / en gran parte falso** | **Rechazado en bloque** | Comprobado contra arXiv: `2202.10868` es *"Neural Program Repair: Systems, Challenges and Solutions"* (Zhong, Li, Ge, Luo), no "A Survey / Xin et al."; `2312.16652` es *"Invariant-based Program Repair"* (Al-Bataineh, FASE 2024, invariants formales para performance bugs), no "Wang et al." sobre features AST/CFG; `1901.01808` es *SequenceR* (Chen et al., IEEE TSE 2019), no "Long & Rinard". `2602.23647` está fechado como 2024 pero el ID implica 2026-02: imposible. Además cita `06_CICD_DEPLOY/WASM_OPT_HARDENING.md`, ruta que **no existe** en esa rama. Se conserva sólo la lista de enlaces oficiales en `docs/REFERENCES.md`. |
| 25 | Riqueza real de `FeatureVector[64]` | Glosario kilo afirma "64 features incluyen patrones AST, CFG y data flow" | `feature_engine`: 0–31 léxicos/hash, 32–47 derivados de hash, 55 duplica 5, 60 = versión de esquema, 63 = bias. **48–54, 56–59, 61–62 quedan en 0.0** | **Repo gana** | `DIM=64` es correcto y no cambia (PROMPT_PAD regla 2), pero ~20 de las 64 entradas son hoy inertes. La afirmación de features estructurales no se sostiene hasta que exista el extractor AST/CFG. Sin impacto en el conteo, sí en la expectativa de calidad del modelo. |
| 26 | Build WASM roto | `wasm.yml` fallaba desde antes de la unificación | **Corregido** en `acddb02` | Repo gana (ya arreglado) | `repair_nn_core` es `no_std` pero llamaba `f32::exp()` (método de `std`) en `sigmoid()` y `soft_argmax()`. Compilaba bajo el feature `std` (por eso `cargo test` salía verde y lo ocultaba) y fallaba en `wasm32-unknown-unknown` con `error[E0599]: no method named exp`. Fallaba también en `5511f61` y `a886669`, o sea que es anterior a esta unificación. Arreglado con `libm::expf`. |

| 27 | Cobertura de `wasm.yml` | Rutas del disparador: `repair_nn_wasm/**`, `repair_nn_core/**`, `repair_types/**` | Faltan `feature_engine/**` y `repair_operators/**` | **Pendiente** | Un cambio en `feature_engine` o `repair_operators` no dispara el build WASM aunque ambos entren en la cadena edge→WASM. No bloquea hoy; anotado para WHEN. |


## Estado

Unificación de ramas (2026-10-01):
- [x] PART1–4 + INDEX + MEM0_STATUS + GOVERNANCE enriquecidos
- [x] Delta útil de `feat/rust-nn-core` extraído (`PipelinePhase`, `verify_result`, `NO_LLM_POLICY.md`, `E2E_CHECKLIST.md`)
- [x] Delta útil de `kilo/bionic-owl-ok9` extraído y depurado (`REFERENCES.md`)
- [x] `WEIGHT_COUNT` corregido a 2863 en las 4 fuentes
- [x] Las 9 ramas residuales eliminadas; **main es la única rama** (`origin`)

Sin resolver (no bloquean la unificación):
- [ ] Ítem 25: ~20 slots de `FeatureVector[64]` inertes hasta que exista extractor AST/CFG
- [ ] Ítems 9, 10, 21: R2/D1/DO, driver Rust de MongoDB y conector Mem0 siguen sin implementar
