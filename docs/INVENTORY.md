# INVENTORY — post-unificación (2026-10-01)

Estado real de `main` tras absorber el delta útil de las 9 ramas y borrarlas.

## Ramas

Verificado 2026-10-01: **1 rama local, 1 rama en `origin`, 0 PRs abiertos, 0 tags.**
(desfasado: los PRs #18–#20 dejaron ramas residuales efímeras 100% fusionadas;
el estado vivo lo comprueba `verify_repo.py` claim `BRANCH_DRIFT`, y se limpia
con el workflow `cleanup-branches.yml`)
Detalle y reglas: `docs/BRANCH_POLICY.md`.

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
├── DISCREPANCIES.md           ← 68 ítems, todos con decisión
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

- `crates/` — 6 miembros: types, feature_engine, nn_core, nn_wasm, operators, train.
- `worker/` — runtime PART3 async (webhook fail-closed → DO → Queue → consumidor), **fuera** del workspace Cargo (paquete CF aparte,
  se compila con wrangler).
- `model/` — `current.txt` = payload KV real (2863 `f32`, pesos V1). `current.json` y `stable.json` siguen como placeholders de metadata (`weights: null`, nadie los consume).
- `legacy/` — archivado, no ejecutable. Se conserva íntegro.
- `FeatureVector::DIM = 64`, `WEIGHT_COUNT = 2863`, gate `0.55 / 0.45`,
  `OperatorId` 0–12 (`OPERATOR_COUNT = 13`).

## Verificación (histórica, CI de `acddb02`; la actual es `ci.yml` con
`workspace-*` + `worker-*` + `validate`, incluyendo los tests de `repair_train`)

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
| 25 | **Parcialmente cerrado por el encoder V1**: los 64 slots llevan señal explícita y un test lo comprueba sobre 1000 muestras sintéticas. Lo que sigue faltando es la señal de AST/CFG, que sigue sin extraerse. |
| — | El webhook ya corre `Incident → features → NN → gate` y falla cerrado sin secret. Falta: abrir PR efímera, persistir `RepairCase`, y `apply()` sigue sin generar diff real. |
| 9, 10 | Sin bindings R2/D1/DO, sin driver Rust de MongoDB, sin conector Mem0. Diseñado, no implementado. |
| 5, 6, 22 | **Train + export cerrados (2026-10-03)**: `crates/repair_train` (trainer offline V1, sin LLM ni `rand`) + `model/current.txt` (payload KV validado en CI con el `extract`/`predict` reales). Pendiente: promoción **humana** a `MODEL_KV` (`model/current`/`model/stable`); un wrapper `scripts/train` sigue sin existir (el bin `repair-train` lo cubre). |
| — | `apply()` tiene un brazo por operador y devuelve `CandidatePatch` (files + steps), pero no genera diff ni escribe ficheros. |
| — | E2E completo no verde (ver `docs/E2E_CHECKLIST.md`). |
| 7, 52 | Sin `Cargo.lock`, **por decisión** (ítem 52): las versiones resuelven en cada build de CI. Cuatro ramas pidieron versionarlos; se explica en DISCREPANCIES por qué no se adoptan tal cual. |
| 34 | Parcial: clippy con `-D warnings` activo y **verde** (los seis checks de `ci.yml` pasan en el PR #13); falta `cargo fmt --check` bloqueante, que necesita toolchain local para aplicar el formato. |
| 51 | **Resuelto** en la unificación del 2026-10-02: el worker no compilaba (lifetime en `model.rs`, import sin usar en `incident_state.rs`) y `repair_nn_wasm` no pasaba clippy. Los tres errores del log del run `36963179520` están corregidos. |
| 41 | **Deploy bloqueado**: sin `CLOUDFLARE_API_TOKEN`/`ACCOUNT_ID` y sin toolchain Rust en el entorno. `worker/wrangler.toml` ya enlaza el namespace real de `MODEL_KV` (el `REPLACE_WITH` fue reemplazado en la auditoría del 2026-10-03). **Nada desplegado.** Vía lista en `.github/workflows/deploy.yml` (manual + environment `production` + preflight + smoke test). |
| 56 | Workers Builds rojo desde al menos `f7522c8`: falla en 0s, o sea configuracion del build en el dashboard de Cloudflare, no codigo. El build de GitHub Actions sobre `wasm32-unknown-unknown --release` pasa. Arreglo: accion humana en el dashboard. |
| — | **λ fijadas (2026-10-03)**: λ_conf = λ_risk = 0.5 en `crates/repair_train`, implementadas (BCE sobre las cabezas de conf/risk) y testeadas. `reward` sigue sin calcularse en el repo (bucle online = fase posterior). |

## Decisiones clave

Repo = fuente de verdad del código.
Notion = diseño/histórico (actualizar sección estado).
PART*.md = diseño consolidado y conciso.
`PROMPT_PAD.md` + `DISCREPANCIES.md` + `INVENTORY.md` = fuente de verdad de diseño.
Una sola rama persistente: main.
Ningún success se declara por confidence del modelo: la autoridad es GitHub Actions.
## Actualización 2026-10-03 (auditoría a profundidad)

- **Ramas**: `origin` solo tiene `main` (3 residuales 100% fusionadas borradas;
  SHAs `cb889cf`, `e781600`, `493594e`). Una sola línea.
- **Reparado en `7dd891f`** (worker): señales anti-loop 3 y 4 reales
  (antes hardcodeadas a 0, config muerta), quota por intentos reales
  (antes COUNT sobre PK: inalcanzable), hard stop de cola (rama inalcanzable
  eliminada; tope real = `max_retries=3` → DLQ + `/attempt` del DO), guard
  `correlation_stale_or_missing` en `/attempt`/`/result`/`/poison`,
  `from_state` real en auditoría, TTL de idempotencia + retención (24 h /
  7 días), `idem_key` canónica del contrato, cola real en `record_poison`.
  Detalle: `DISCREPANCIES` 58–66.
- **Docs sincronizadas** (GOVERNANCE, CONTRACT §2, PART3 §8/§9/§10/§12/§13,
  E2E_CHECKLIST, ARCHITECTURE, PHASE_STATUS, BRANCH_POLICY) y
  `verify_repo.py` ahora cubre `deploy-staging.yml`.
- Verificación: `cargo test --workspace`, clippy `-D warnings`, worker
  host + `wasm32-unknown-unknown --release`, `verify_repo.py` OVERALL PASS.

## Actualización 2026-10-03 (tarde): pesos V1 entrenados

- **Entrenamiento offline V1 implementado**: `crates/repair_train` (SGD
  determinista sobre el dataset sintético, sin `rand`; λ_conf = λ_risk = 0.5
  **fijadas** — cierra el gap de λ) + bin `repair-train`
  (`cargo run -p repair_train --release -- --out model/current.txt`).
- **`model/current.txt`**: payload KV real (2863 `f32`, formato exacto del
  loader del worker). Corrida V1 (dataset 1000/seed 42, 120 épocas, batch 16,
  lr 0.1→0.03, init seed 7): accuracy 1.000 observada en train y holdout
  (500, seed 43); CI revalida el artefacto con el `extract`/`predict` reales
  (tests de `repair_train`).
- **No promovido**: `MODEL_KV` sigue sin las claves `model/current`/
  `model/stable` → el Worker responde `blocked_no_model`. Promoción =
  acción humana; nada se declara PASS sin GitHub Actions.
- Los placeholders `current.json`/`stable.json` se conservan (metadata, nadie
  los consume).

## Actualización 2026-10-03 (docs sync tras auditoría doc↔código)

- Auditoría completa doc↔código (20 docs vs código en `4dafd33`): corregidas las
  afirmaciones desfasadas de PART2 / INDEX / INVENTORY-41 / BRANCH_POLICY, y
  añadido `repair_train` a los layouts de PART1 / PROMPT_PAD (ítems 69–70 de
  DISCREPANCIES).
- `verify_repo.py` ahora escanea también `promote-model.yml` y
  `cleanup-branches.yml` (`WORKFLOW_LEAST_PRIVILEGE`).

## Actualización 2026-10-03 (GATE→PR V1)

- **P2 de `CONTRACT.md` §3 cerrado como crate**: `crates/repair_pr` — generador
  de diff unificado real (`similar` 3.2.0) + apertura de PR (`octocrab` 0.54.2
  con `secrecy` 0.10.3; versiones verificadas contra crates.io). 7º miembro del
  workspace; bin `repair-pr` (`diff` offline | `pr` desde rama efímera).
- **Fail-closed de extremo a extremo**: token solo de `GITHUB_TOKEN` (jamás
  adivinado ni logueado); bundle vacío ⇒ `Blocked` (NO INVENTED DIFFS); PR
  duplicado ⇒ `ReturnExisting` (CONTRACT §5); nunca declara PASS (§4: la
  autoridad de VERIFY es GitHub Actions).
- **Sin integración aún (honesto)**: el worker no invoca `repair_pr`; la
  conversión `ops::CandidatePatch` → `contract::CandidatePatch` corre por
  cuenta del caller; `repair_operators::apply()` no cambia.
