# INVENTORY — post-unificación (2026-10-01)

Estado real de `main` tras absorber el delta útil de las 9 ramas y borrarlas.

## Ramas

- **main** — única rama persistente.
- ~~setup/secrets~~, ~~feat/rust-wasm-nn~~, ~~fix/safejson-null-payload~~ —
  ancestros lineales de `main`. `git diff origin/main...origin/<rama>` vacío.
  Sin delta que extraer.
- ~~chore/unify-docs-cleanup~~, ~~chore/enrich-docs-from-kilo~~ — su contenido
  ya estaba en `main` (PART1–4, INVENTORY, INDEX, MEM0_STATUS, GOVERNANCE
  idénticos o superados; los ítems 18/19 de DISCREPANCIES ya estaban).
- ~~feat/rust-complete-stack~~, ~~docs/prompt-pad-part1-2~~ — originales
  verbosos de PART1–4 (121–220 líneas). Los condensados de `main` preservan
  todas las decisiones (rechazo de Emscripten, límites Free, esqueleto de
  `wrangler.toml`, matriz de separación de memoria, regla Context7).
  Sin pérdida de información crítica.
- ~~feat/rust-nn-core~~ — único portador de delta real. Extraído:
  - `crates/repair_types`: `PipelinePhase` ampliado + `verify_result` en
    `PipelineReport` (commit `b3c4046`).
  - `docs/NO_LLM_POLICY.md` (de `HYBRID_POLICY.md`), renombrado porque el
    documento nunca propuso híbrido: lo rechaza.
  - `docs/E2E_CHECKLIST.md`.
  - **No absorbido**: `PONY.md` (ya excluido por `docs/AGENTS.md`), y el
    código de esa rama que `main` ya supera (`worker/src/lib.rs` con Router y
    rutas `/health`, `/model`, `/webhook` separadas; `Cargo.toml` sin
    `thiserror` ni `wasm-bindgen` sin uso).
- ~~kilo/bionic-owl-ok9~~ — 30 archivos / +7634. Absorbido **depurado**:
  `docs/REFERENCES.md` (enlaces oficiales + glosario corregido).
  Descartados: `DOCUMENTATION.md` (afirma "MongoDB ✅ Conectado" sin driver,
  ítem 21) y `ACADEMIC_REFS.md` (citas académicas falsificadas, ítem 24).

## Árbol relevante (docs)

```
docs/
├── ARCHITECTURE.md            ← flujo + aritmética de pesos
├── PROMPT_PAD.md              ← contrato de implementación (SoT)
├── DISCREPANCIES.md           ← 25 ítems, todos con decisión
├── INVENTORY.md               ← este archivo
├── GOVERNANCE.md              ← SoT umbrales 0.55 / 0.45
├── NO_LLM_POLICY.md           ← por qué no hay LLM ni fallback híbrido
├── E2E_CHECKLIST.md           ← FASE 9 + estado real por paso
├── REFERENCES.md              ← enlaces oficiales + glosario
├── PART1_REPOSITORY.md
├── PART2_NEURAL_NETWORK.md
├── PART3_CLOUDFLARE_RUNTIME.md
├── PART4_PERSISTENCE_TRANSVERSAL.md
└── AGENTS.md, CODEMAP.md, INDEX.md, PHASE_STATUS.md, MEM0_STATUS.md,
    BRANCH_POLICY.md, REPAIR_PROTOCOL.md, ROOT_LAYOUT.md, WASM.md
```

## Código

- `crates/` — 5 miembros: types, feature_engine, nn_core, nn_wasm, operators.
- `worker/` — esqueleto, **fuera** del workspace Cargo (paquete CF aparte,
  se compila con wrangler).
- `model/` — placeholders. `current.json` y `stable.json` con `weights: null`.
- `legacy/` — archivado, no ejecutable. Se conserva íntegro.
- `FeatureVector::DIM = 64`, `WEIGHT_COUNT = 2863`, gate `0.55 / 0.45`,
  `OperatorId` 0–12 (`OPERATOR_COUNT = 13`).

## Verificación (CI en `acddb02`, todas en verde)

| Workflow | Resultado |
|----------|-----------|
| CI / test | ✅ `cargo test -p repair_types -p feature_engine -p repair_nn_core -p repair_operators` — 27 tests |
| CI / worker | ✅ `cargo check --manifest-path worker/Cargo.toml --all-targets` (worker enlaza los 4 crates) |
| CI / clippy | ✅ `-D warnings` |
| WASM | ✅ `cargo build -p repair_nn_wasm --release --target wasm32-unknown-unknown` |
| Security | ✅ |

El build WASM estaba roto desde antes de la unificación (`f32::exp()` en un
crate `no_std`); corregido en `acddb02` con `libm::expf`. Ver `DISCREPANCIES` 26.

## Gaps vivos tras la unificación

| # | Gap |
|---|------|
| 25 | ~20 de las 64 entradas de `FeatureVector` están inertes (0.0). Sin extractor AST/CFG. |
| — | El webhook ya corre `Incident → features → NN → gate` y falla cerrado sin secret. Falta: abrir PR efímera, persistir `RepairCase`, y `apply()` sigue sin generar diff real. |
| 9, 10 | Sin bindings R2/D1/DO, sin driver Rust de MongoDB, sin conector Mem0. Diseñado, no implementado. |
| 5, 6, 22 | Sin pesos reales: no hay `scripts/train` ni `scripts/export_weights`; training y promoción de checkpoint siguen diferidos. |
| — | `apply()` tiene un brazo por operador y devuelve `CandidatePatch` (files + steps), pero no genera diff ni escribe ficheros. |
| — | E2E completo no verde (ver `docs/E2E_CHECKLIST.md`). |
| — | Sin `Cargo.lock` (ítem 7): las versiones resuelven en cada build de CI. |
| 34 | Parcial: clippy con `-D warnings` activo; falta `cargo fmt --check` (necesita toolchain local). |
| 41 | **Deploy bloqueado**: sin `CLOUDFLARE_API_TOKEN`/`ACCOUNT_ID` y sin toolchain Rust en el entorno. `wrangler.toml` conserva `REPLACE_WITH_KV_NAMESPACE_ID`. **Nada desplegado.** Vía lista en `.github/workflows/deploy.yml` (manual + environment `production` + preflight + smoke test). |
| — | Objetivo de entrenamiento sin λ fijados: la fórmula loss/reward está documentada en PART2 pero no implementada, y nada calcula `reward` en el repo. |

## Decisiones clave

Repo = fuente de verdad del código.
Notion = diseño/histórico (actualizar sección estado).
PART*.md = diseño consolidado y conciso.
`PROMPT_PAD.md` + `DISCREPANCIES.md` + `INVENTORY.md` = fuente de verdad de diseño.
Una sola rama persistente: main.
Ningún success se declara por confidence del modelo: la autoridad es GitHub Actions.