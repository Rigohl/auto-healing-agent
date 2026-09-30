# DISCREPANCIES — FASE 1 (2026-09-30)

Regla: cada ítem tiene decisión. Sin decisión documentada → bloqueante.

| # | Elemento | Notion | Repo (`main`) | Decisión | Justificación |
|---|----------|--------|--------------|---------|-------------|
| 1 | Dims del FeatureVector | Snippet 16; sección estado dice 16→32→11 (907 pesos) | **64**, schema v2, 2617 pesos (`repair_types::FeatureVector::DIM`, `model/schema.json`) | **Repo gana: 64-dim** | `docs/PROMPT_PAD.md` regla 2: ante conflicto de dims, 64. El snippet 16 es código de referencia inicial ya superado; la sección "Estado de implementación" de Notion describe el PR #1 pre-merge y no se actualizó. |
| 2 | `package.json` / runtime Node | Implícito (agent.ts en raíz, workflows con npm) | **No existe** (code search = 0 hits) | **Rust-only en producción** | README y PROMPT_PAD lo declaran. `legacy/agent.ts` conserva el path TS como archivo muerto. |
| 3 | Path LLM (`agent.ts` + HF) | Política híbrida: "si no actionable → agent.ts + HuggingFace"; cita `docs/HYBRID_POLICY.md` | `legacy/agent.ts` movido; `auto-repair.yml` es solo notice; **`docs/HYBRID_POLICY.md` NO EXISTE en el repo** (404) | **Rust-only. Híbrido descartado por ahora** | Decisión ya tomada en repo (DISCREPANCIES previo + README). La cita de Notion a HYBRID_POLICY.md es falsa — hay que corregirla en Notion, no recrear el archivo. |
| 4 | `tests/` directorio | Estructura propuesta: `tests/{neural,operators,fixtures,regression}/` | **No existe** (404); tests inline en crates | **Mantener tests inline; crear `tests/` solo si hace falta integración** | Los 4 crates ya tienen `#[cfg(test)]` con cobertura básica (dim 64, fixture syntax_error, zeros NN). Directorio vacío añadiría ruido. |
| 5 | Scripts train/export/validate | `scripts/{train_model,export_weights,validate_model}.rs` | Solo `set-github-secrets.sh` | **Diferir; crear `export_weights` cuando haya pesos reales** | `model/current.json` es placeholder. Sin pesos entrenados no hay qué exportar. Burn queda como candidato offline (Notion + PROMPT_PAD). |
| 6 | `model/README.md` | En estructura propuesta | 404 | **Crear en FASE 5** junto al primer export real | Depende del item 5. |
| 7 | `LICENSE` / `Cargo.lock` | En estructura propuesta | Ambos 404 | **Añadir `Cargo.lock` (git)**; `LICENSE` ya declarado MIT en workspace | Cargo.lock versiona deps reproducibles; es práctica estándar. LICENSE: el workspace ya dice MIT, falta el archivo. |
| 8 | Worker cableado a NN | Flujo: Worker ejecuta WASM (paso 4) | `worker/src/lib.rs` solo health/KV/webhook-accept; **no importa repair_nn_*** | **Cablear en FASE 6**: Worker → feature_engine → RepairNet (o WASM) → gate | El skeleton es intencional (PROMPT_PAD FASE 6: "worker Rust health+KV read"). No es bug, es fase pendiente. |
| 9 | Memoria: Mem0 / R2 / D1 / DO | Diseño completo en página | **Cero** en código; wrangler solo KV MODEL_KV con IDs placeholder | **KV primero (pointers), R2 cuando haya artefacto WASM real; Mem0/D1/DO diferidos** | Free tier: no provisionar lo que no se usa. PROMPT_PAD ya limita CF a webhook/router/features/inference/gate/KV/R2. |
| 10 | MongoDB en el núcleo Rust | Persistencia de RepairCase | Sin driver Mongo en crates; solo `legacy/mongodb-setup.sh` (postmortems) | **Persistencia Rust diferida; MongoDB sigue siendo SoT de memoria histórica** | El diseño (Notion) pone MongoDB al final del pipeline. Implementarlo sin el resto del flujo es prematuro. |
| 11 | Umbrales confidence/risk | Governance en página (sin números explícitos allí) | `docs/GOVERNANCE.md`: MIN_CONFIDENCE=0.55, MAX_RISK=0.45 | **Repo: 0.55 / 0.45** | Números versionados en repo; Notion debería reflejarlos. |
| 12 | Operadores | Notion: 5 outputs (NO_ACTION..SOURCE_REPAIR) | `OperatorId` 0..12 (13 ops) + Unknown | **Repo: 13 ops** | El enum creció con el diseño (SyntaxFix, BuildScriptFix, EnvVarRepair...). Notion desactualizado. |
| 13 | `should_fallback_to_llm` | Notion tabla crates: "stubs + should_fallback_to_llm" | **No existe** en `repair_operators` | **No reintroducir** | Contradice la política rust-only (README, PROMPT_PAD behavioral #6). La tabla de Notion describe un estado intermedio ya superado. |
| 14 | Páginas Notion duplicadas | Dos páginas con el mismo título + "(1)" | N/A | **Conservar la canónica** (`3eb50567-40be-8146-b75a-e90f6f818580`, 12:52 UTC) y archivar/eliminar la `(1)` | Duplicado mecánico; ambas tienen el mismo cuerpo desactualizado. |
| 15 | Sección "Estado de implementación" en Notion | Describe PR #1 (16-dim, 907 pesos, híbrido) | Repo: 64-dim, 2617 pesos, rust-only, PR merged | **Actualizar esa sección en Notion** con los números reales | Es la única parte de Notion que afirma estado de código; está mal y genera confusión en futuras fases. |

## Gate de salida FASE 1

- [x] Discrepancias 1–15 con decisión y justificación cada una.
- [x] Ausencias verificadas con API (404) y code search, no por asunción.
- [x] `docs/INVENTORY.md` y `docs/DISCREPANCIES.md` en el repo.
- [x] `docs/ARCHITECTURE.md` alineado con código leído.

**FASE 1: COMPLETED.** Siguiente: FASE 2 (representación) — FailureSignature + features 64 + AST stubs.
