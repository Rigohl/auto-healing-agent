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
| 20 | WEIGHT_COUNT | Docs del repo decían 2617 | **2863** (código) | **Repo código gana** | `crates/repair_nn_core/src/lib.rs:18`: `64*32+32 + 32*16+16 + 16*13+13 + 16+1 + 16+1` = 2863. El 2617 era una cifra heredada de u
na variante de arquitectura anterior y no correspondía a ningún código. Corregido en ARCHITECTURE.md, PART2, INVENTORY.md y este ítem 1. |
| 21 | MongoDB "✅ Conectado" | Origen: PDF de diseño PART1–4 (*"MongoDB Atlas Ñ estado verificado"*) y `kilo/bionic-owl-ok9:docs/DOCUMENTATION.md` (*"MongoDB Atlas ✅ Conectado (Cluster0)"*) | Sin driver Rust en código | **Rechazado** | Constraint: no declarar_SUCCESS ni inventar conectores. MongoDB sigue diferido (ítem 10). El resto de ese doc de 491 líneas se descartó por este motivo. La colección `audit/incidents/postmortems/repair_rules/training_examples` del PDF es **diseño de Notion, no estado del repo**: no hay driver Rust (ítem 10) y el único incidente citado (`syntax_error`, `buildStep`, `npm run vercel-build`, 3 intentos, `repair_failed`) se conserva en PART4 sólo como **material histórico para el dataset**, no como verificación. |
| 22 | Nº de agentes lógicos | `AGENTS.md` = 14 · `HYBRID_POLICY.md` = 8 | Código: `PipelinePhase` tiene 15 variantes | **Grano de diseño, no de código** | Sin contradicción funcional: los agentes son fases del pipeline y el código sólo codifica fases. `NO_LLM_POLICY.md` conserva la lista de 8 y señala `AGENTS.md` como desglose fino de 14. No bloquea. |
| 23 | Pony / actor runtime | `feat/rust-nn-core:docs/PONY.md` (FASE 8) | `docs/AGENTS.md` ya dice *"Pony / actor runtime nativo: fuera de este repo"* | **Descartado** | El propio AGENTS.md lo excluye. PONY.md no se absorbe; su decisión ("not started by design order") ya está registrada allí. |
| 24 | Referencias académicas kilo | `ACADEMIC_REFS.md` cita 9 papers con títulos, autores y hallazgos | **No verificable / en gran parte falso** | **Rechazado en bloque** | Comprobado contra arXiv: `2202.10868` es *"Neural Program Repair: Systems, Challenges and Solutions"* (Zhong, Li, Ge, Luo), no "A Survey / Xin et al."; `2312.16652` es *"Invariant-based Program Repair"* (Al-Bataineh, FASE 2024, invariants formales para performance bugs), no "Wang et 
al." sobre features AST/CFG; `1901.01808` es *SequenceR* (Chen et al., IEEE TSE 2019), no "Long & Rinard". `2602.23647` está fechado como 2024 pero el ID implica 2026-02: imposible. Además cita `06_CICD_DEPLOY/WASM_OPT_HARDENING.md`, ruta que **no existe** en esa rama. Se conserva sólo la lista de enlaces oficiales en `docs/REFERENCES.md`. |
| 25 | Riqueza real de `FeatureVector[64]` | Glosario kilo afirma "64 features incluyen patrones AST, CFG y data flow" | `feature_engine`: 0–31 léxicos/hash, 32–47 derivados de hash, 55 duplica 5, 60 = versión de esquema, 63 = bias. **48–54, 56–59, 61–62 quedan en 0.0** | **Repo gana** | `DIM=64` es correcto y no cambia (PROMPT_PAD regla 2), pero ~20 de las 64 entradas son hoy inertes. La afirmación de features estructurales no se sostiene hasta que exista el extractor AST/CFG. Sin impacto en el conteo, sí en la expectativa de calidad del modelo. |
| 26 | Build WASM roto | `wasm.yml` fallaba desde antes de la unificación | **Corregido** en `acddb02` | Repo gana (ya arreglado) | `repair_nn_core` es `no_std` pero llamaba `f32::exp()` (método de `std`) en `sigmoid()` y `soft_argmax()`. Compilaba bajo el feature `std` (por eso `cargo test` salía verde y lo ocultaba) y fallaba en `wasm32-unknown-unknown` con `error[E0599]: no method named exp`. Fallaba también en `5511f61` y `a886669`, o sea que es anterior a esta unificación. Arreglado con `libm::expf`. |

| 27 | `wasm.yml` path filter | No cubría `feature_engine` ni `repair_operators` | Cubierto + `worker/**` | **Corregido** | Ambos crates entran en la cadena edge→WASM y un cambio ahí no disparaba el build WASM. |
| 28 | `worker/` sin verificar | Ningún workflow lo compilaba | Ahora `ci.yml` job `worker` | **Corregido** | `worker/` está fuera del workspace Cargo (paquete CF aparte), así que `cargo test`/`cargo build` del workspace nunca lo compilaban: `worker/src/lib.rs` y la dep `worker = "0.5"` podían romperse con CI en verde. Añadido `cargo check --manifest-path worker/Cargo.tom
l --all-targets`. |
| 29 | `repair_operators` sin tests | Crate del gate 0.55/0.45 con **0 tests** | 15 tests inline | **Corregido** | El gate es el path más crítico del sistema y era el único crate sin cobertura. Cubierto: inclusión exacta en ambos umbrales, bloqueo 0.5499 y 0.4501, NoOp/Unknown con confidence 1.0, monotonicidad, `verify_result == None`, razón de denegación, allowlist de los 13 operadores y coherencia gate↔apply. |
| 30 | Test de `WEIGHT_COUNT` débil | `assert!(WEIGHT_COUNT > 1000)` | `assert_eq!(WEIGHT_COUNT, 2863)` + test de aritmética por capas | **Corregido** | Una aserción que acepta 2617 y 2863 por igual es la razón de que el error de documentación del ítem 20 pasara desapercibido. Añadido también `from_weights` rechaza longitud incorrecta. |
| 31 | `regression.yml` sin `repair_types` | Corría 3 crates, CI corría 4 | Alineado con `ci.yml` | **Corregido** | La regresión semanal no ejecutaba los tests de umbral del gate. |
| 32 | Script de secrets reintroducía HF | `set-github-secrets.sh` subía `HF_TOKEN`, `HF_MODEL` (default `Qwen/Qwen2.5-Coder-7B-Instruct`), `HF_BASE_URL` (default router HF) y `VERCEL_ORG_ID`/`VERCEL_PROJECT_ID` embebidos | Eliminados | **Corregido** | El script es la ruta viva de aprovisionamiento (no está en `legacy/`) y creaba por defecto el stack HuggingFace que el repo declara retirado (ítems 3 y 13), además de subir org/project ids reales sin que el operador los exportara. Añadido `WEBHOOK_SECRET`. `put_secret` ya descartaba valores vacíos correctamente: no había bug de borrado. |
| 33 | Ausentes `.gitignore` y `LICENSE` | No existían | Ambos añadidos | **Corregido** | Sin `.gitignore`, `target/`, `worker/build/`, `pkg/` y `.env` podían commitearse. Todos los `Cargo.toml` declaraban `license = "MIT"` sin que existiera el archivo (ítem 7). |
| 34 | Sin `cargo fmt --check` en CI | `rust-toolchain.toml` declara `rustfmt` y `clippy`; ningún workflow los usa | clippy añadido (informativo, sin `-D warnings`); fmt pendiente | 
**Parcial** | No se puede aplicar `rustfmt` en este entorno, así que activar `--check` dejaría CI en rojo sin forma de corregirlo aquí. Requiere una pasada con toolchain local. |
| 35 | `worker/` no compilable | `worker/Cargo.toml` sin tabla `[workspace]` | Añadida | **Corregido** en `f8d9f79` | Cargo lo rechazaba en standalone: *"current package believes it is in a workspace when it is not"*. No era solo un problema de CI: `worker-build` es lo que invoca `wrangler.toml` para desplegar, así que **el build de Cloudflare estaba roto** y el worker no se había podido construir nunca desde ese manifest. Descubierto al añadir el job `worker` del ítem 28. |
| 36 | "Evidencia académica" en PDF de PART2 | El PDF de diseño afirma *"Modelos pequeños y simples → mejor relación coste/precisión en edge restringido"* sin citar paper verificable | Rechazado, igual que el ítem 24 | **Coincide con 24** | El PDF es el **origen aguas arriba** de la fabricación que ya se había rechazado en `ACADEMIC_REFS.md` de `kilo/bionic-owl-ok9`. Se conserva la decisión de diseño (red pequeña para edge) por su propia cuenta, sin atribuirle respaldo académico. |
| 37 | `apply()` con catch-all | `other =>` cubría OperatorId 4–12 | Un brazo explícito por variante, sin default | **Corregido** | Añadir una variante al enum compilaba y producía un parche genérico. Ahora `apply()` es exhaustivo: añadir un `OperatorId` rompe el build. `EnvVarRepair` y `CacheClear` pasan por `escalate()`, que por construcción no toca ficheros. Los operadores 4, 6, 7, 8, 9, 11, 12 pasaron de "allowlisted transform only" a ficheros y pasos concretos. |
| 38 | Webhook sin pipeline | `let _body = req.text()` descartaba el body y devolvía respuesta fija | 5 pasos reales hasta el gate | **Corregido** | `Incident` → `FailureSignature` → `extract` → `RepairNet::predict` → `gate`, respondiendo con el `PipelineReport`. Sin `unwrap()` en el path. PR sigue ausente y la respuesta lo declara (`"pr": null`). |
| 39 | Webhook fail-open | Si
 faltaba `WEBHOOK_SECRET` se saltaba la comprobación | 503 si no está configurado | **Corregido** | En un path que pronto abrirá PRs, aceptar tráfico sin secret es fail-open. |
| 40 | Pesos malformados | Sin validación | Archivo entero rechazado | **Corregido** | `load_weights` exige exactamente `WEIGHT_COUNT` f32 finitos separados por espacio. Un token no numérico invalida todo el archivo en vez de ignorarse, porque ignorarlo dejaría la red con pesos desplazados. Sin pesos válidos: **BLOCKED** (`blocked_no_model`); red de ceros prohibida (`model.rs`: "Nunca: current failure -> zeros -> PASS"). (Corrección 2026-10-03: el texto anterior decía "cae a ceros y lo declara", desfasado de la política actual.) |
| 41 | Deploy a Cloudflare | — | **No ejecutable desde el entorno actual** | **Bloqueado, no fallido** | Faltan tres cosas verificables: (a) no hay `CLOUDFLARE_API_TOKEN` ni `CLOUDFLARE_ACCOUNT_ID` — las variables `CLOUDFLARE_*` presentes son la infraestructura del sandbox, **no** credenciales de la cuenta; (b) no hay toolchain Rust (`cargo`/`rustc` ausentes), y `wrangler deploy` invoca `cargo install worker-build && worker-build --release`; (c) `worker/wrangler.toml` conserva `REPLACE_WITH_KV_NAMESPACE_ID`. **No se ha desplegado nada y no se declara despliegue exitoso.** Se deja la vía preparada y verificable: `deploy.yml` (solo `workflow_dispatch`, environment `production`, preflight que aborta si faltan secrets o hay `REPLACE_WITH_*`, y smoke test que verifica /health, el fail-closed sin secret y el pipeline con secret) y un job de CI que compila el worker para `wasm32-unknown-unknown`, que es el artefacto real de deploy. |


## Estado

Unificación de ramas (2026-10-01):
- [x] PART1–4 + INDEX + MEM0_STATUS + GOVERNANCE enriquecidos
- [x] Delta útil de `feat/rust-nn-core` extraído (`PipelinePhase`, `verify_result`, `NO_LLM_POLICY.md`, `E2E_CHECKLIST.md`)
- [x] Delta útil de `kilo/bionic-owl-ok9` extraído y depurado (`REFERENCES.md`)
- [x] `WEIGHT_COUNT` corregido 
a 2863 en las 4 fuentes
- [x] Las 9 ramas residuales eliminadas; **main es la única rama** (`origin`)

Sin resolver (no bloquean la unificación):
- [x] Ítem 28: `worker/` ya se compila en CI
- [x] Ítem 29: `repair_operators` con 15 tests del gate
- [x] Ítems 30, 31: aserción de pesos y `regression.yml` alineados
- [x] Ítems 32, 33: script de secrets sin HF y sin defaults embebidos; `.gitignore` + `LICENSE`
- [x] Ítem 35: `worker/` compilable (build de Cloudflare estaba roto)
- [x] Ítem 36: PDF de diseño PART1–4 cotejado; origen aguas arriba de los ítems 21 y 24

Sin resolver:
- [x] Ítem 25: el encoder V1 rellena los slots 48–62 (bucetos hash) y un test comprueba que no hay slots constantes en 1000 muestras sintéticas. Queda abierto solo el extraer señal AST/CFG real.
- [x] Ítem 27: `wasm.yml` cubre `feature_engine/`, `repair_operators/` y `worker/`
- [x] Ítems 37-40: allowlist exhaustivo, pipeline del webhook, fail-closed, validación de pesos
- [ ] Ítem 41: deploy a Cloudflare bloqueado (sin credenciales ni toolchain). Vía preparada en `deploy.yml`; **nada desplegado**
- [x] Ítem 34 (parcial): clippy con `-D warnings`; sigue faltando `cargo fmt --check`
- [ ] Ítems 9, 10, 21: R2/D1/DO, driver Rust de MongoDB y conector Mem0 siguen sin implementar
- [ ] Sin `Cargo.lock` (ítem 7): las versiones resuelven en cada build de CI
- [ ] Pipeline no cableado: el webhook descarta el body; `apply()` no genera diff; `wrangler.toml` sin `[wasm_modules]` y con `REPLACE_WITH_KV_NAMESPACE_ID`

---

## Reconciliacion P1: CI real, gobernanza y PRs #6-#9 (2026-10-02)

Criterio de fase: cerrar/mergear PRs sin dejar main no compilable y documentar toda decision de alcance.

| # | Elemento | Estado encontrado | Decision | Justificacion |
|---|----------|-------------------|----------|---------------|
| 42 | PRs #6-#9 | 4 drafts abiertos, todos `mergeable_state: dirty`, basados en `f7522c83` (era worker 0.5 sincrona) | **Cerrados como superados** (ramas conservadas) | main absorbio sus cam
bios nucleo y evoluciono al runtime PART3 async (`c5fadd4`: Durable Object SQLite, Queues+DLQ, quotas, anti-loop; `d8212dc`: migracion a workers-rs 0.8.7). Mergear cualquiera regresaria el runtime: p.ej. el #8 eliminaria los bindings de Durable Objects y Queues de `worker/wrangler.toml` (-26 lineas). Analisis por PR en el comentario de cierre de cada PR. |
| 43 | `worker = "0.8.x"` | main: `worker = { version = "0.8", features = ["queue"] }` -> resuelve 0.8.7 (`max_stable_version` de crates.io, verificado 2026-10-02) | **Ya resuelto; sin cambio en este PR** | La subida 0.5->0.8.x que proponian #6/#9 ya esta en main, con el feature `queue` que el runtime PART3 necesita. No se edito `worker/Cargo.toml`. |
| 44 | Excepcion de alcance Fase 0 | El alcance nominal de P1 prohibe `worker/src/**` y `model/**`; los PRs #6-#9 los modifican | **Excepcion autorizada solo para merge/cierre; NO ejercida para editar** | Se decidio cerrar sin mergear: ningun fichero de `worker/**`, `crates/**` ni `model/**` fue editado por P1. Ediciones del worker = P2; promocion de `model/` = P4. |
| 45 | CI que no cubria el workspace ni el worker completo | `ci.yml`: listado `-p` selectivo (excluia `repair_nn_wasm`), worker solo `check`, sin fmt, sin test/clippy del worker | **Reescrito con 6 jobs** | `workspace-test`/`workspace-clippy`/`workspace-fmt` + `worker-check`/`worker-test`/`worker-clippy` con `--manifest-path worker/Cargo.toml` (su `[workspace]` propio lo vuelve inalcanzable desde el raiz, item 35). `regression.yml` alineado a `--workspace`. |
| 46 | `cargo fmt` ausente (item 34) | Deuda de formato; activar `--check` dejaria CI rojo sin toolchain local para corregir | **Check real, advisory hoy** | `workspace-fmt` ejecuta `cargo fmt --all -- --check` con `continue-on-error`: resultado visible y veridico (rojo mientras exista deuda), no bloquea. Al ejecutar `cargo fmt --all` sobre el arbol: quitar `continue-on-error` y anadirlo a required checks. Nunca verde falso. |
| 47 | Falso positivo
 del preflight REPLACE_WITH | `deploy.yml` grepea `worker/wrangler.toml` completo, comentarios incluidos | **Filtro de comentarios (absorbido de #8)** | Un comentario que documente el placeholder bloquearia el unico camino de deploy (P0 reportado en el historial). Ahora `sed -e 's/#.*//'` antes del grep. |
| 48 | Branch protection / rulesets | `branches/main/protection` -> 401 sin admin; `rulesets` -> `[]`; `main.protected = false` | **UNVERIFIABLE/ausente; no se afirma enforcement** | Ningun check es obligatorio hoy a nivel de plataforma. Required checks propuestos con nombres exactos en GOVERNANCE.md; activarlos es accion manual humana. |
| 49 | Mergify / CODEOWNERS / dependabot / SECURITY.md | No existen `.mergify.yml`, `CODEOWNERS`, `dependabot.yml`, `SECURITY.md` | **Ausentes; no creados** | P1 no los crea sin pedido humano explicito. Mergify no se introduce (AUTO_MERGE=false). |
| 50 | `VERCEL_ORG_ID` literal en legacy | `legacy/CONFIG.md` contiene un org-id Vercel literal (era V0, archivada) | **Reportado; no rotado** | No se rota ni elimina en silencio: decision humana. `legacy/` no se ejecuta. |
| 51 | Job `worker` rojo en main HEAD (`d8212dc`) | check-runs del HEAD: `worker` = failure; `test`/`clippy`/`build`/`hygiene` = success | **Preexistente, de P2; no ocultado** | El worker de main no compila hoy (deuda activa de P2 tras `2f4b023`/`d8212dc`). La nueva CI lo sigue mostrando en rojo: `worker-check`/`worker-test`/`worker-clippy` fallaran hasta que P2 lo arregle. Este PR no lo enmascara ni lo corrige (fuera de alcance). |
| 52 | `Cargo.lock` sin versionar | Cuatro ramas (`#7`, `#8`, `#9`, `#10`) lo anaden con `--locked` en CI para builds reproducibles | **`main` NO los versiona; se mantiene la decision, documentada** | Un lockfile de esas ramas es anterior a `repair_types::contract` y a `serde_json` como dev-dependency: adoptarlo con `--locked` rompe la resolucion, y sin `--locked` es un archivo que miente sobre las dependencias reales. Ademas fijaria el 
toolchain `stable` a un instante concreto sin decidirlo explicitamente. Como resolverlo exige `cargo generate-lockfile` contra el arbol actual, es una PR propia con su propio `--locked`, no un efecto secundario de esta unificacion. |
| 53 | Contrato GitHub ↔ Cloudflare | `feat/contract-github-cloudflare-4476481194757304259` (PR #11, DRAFT) lo define entero, y su seccion 7 afirma HMAC `x-hub-signature-256` con `subtle::constant_time_eq` | **Se integra el modulo y el doc, con la seccion 7 corregida** | El codigo real verifica un secret compartido (`x-webhook-secret`) en tiempo constante con `runtime::security::verify_webhook_secret`; no hay HMAC ni crate `subtle`. Dejar la afirmacion seria un claim falso sobre la seguridad del endpoint. |
| 54 | Ramas de PR #6–#12 revierten el runtime PART3 | Cuatro ramas modifican `worker/src/lib.rs` para volver al pipeline sincrono V0 | **Se rechaza ese hunk; se acepta el resto de la rama** | Borrarian el Durable Object, la Queue, `#[event(queue)]`, quotas y anti-loop, y cambiarian la comparacion del secret por igualdad simple. Un merge traeria ese rollback por detras del "delta util" que las justificaba. |
| 55 | `repair_nn_wasm` sin `Default` | `clippy::new_without_default` en `RepairModel::new()` era la razon de que `workspace-clippy` estuviese rojo | **Corregido** con `impl Default for RepairModel` (delta de PR #9) | Un modelo sin pesos es exactamente el estado inicial, asi que `default()` y `new()` coinciden. Sin esto, `workspace-clippy` sigue rojo con `-D warnings`. |

### Resultados observados del PR #12 (2026-10-02)

Primera ejecucion de la CI real sobre la rama `chore/p1-ci-real-governance`:

| Check | Resultado | Lectura |
|-------|-----------|---------|
| `workspace-test` | success | El workspace raiz compila y pasa tests. |
| `workspace-clippy` | failure | `repair_nn_wasm` nunca paso por clippy `-D warnings`: el listado `-p` lo exclia. Deuda preexistente ahora visible; arreglarla es edicion de `crates/**` (P2/P4), fuera 
del alcance de P1. |
| `workspace-fmt` | failure (advisory) | Esperado: item 46. Deuda de formato real, no bloquea. |
| `worker-check` | failure | El worker de main no compila (item 51): deuda activa de P2. |
| `worker-test` | failure | Mismo motivo (no compila, no llega a ejecutar tests). |
| `worker-clippy` | failure | Mismo motivo. |

Los rojos son deuda preexistente expuesta por la CI real, no regresiones introducidas por el PR #12. Antes de este PR, cuatro de esas seis senales no existian y la unica que si existia (check del worker) ya estaba roja en el HEAD de main (`d8212dc`).
| 56 | Workers Builds rojo en todos los commits | `failure` en `88ff048`, `abf6566`, `d8212dc`, `f9b2025`, `f7522c8` y en el PR #13 | **Preexistente; no lo introduce esta unificacion** | El check de la integracion de Cloudflare falla en 0s, o sea antes de compilar nada: es configuracion del build en el dashboard, no codigo. El build de GitHub Actions (`worker-check` sobre `wasm32-unknown-unknown --release`) si pasa. Diagnostico y arreglo: accion humana en el dashboard. |
| 57 | `Kilo Code Review` en rojo | `action_required`: "Review could not start because the account has insufficient credits" | **Externo al repo; no bloquea nada** | Es la cuenta de quien lo dispara, no una propiedad del codigo. Se registra para que el rojo no se lea como un fallo de este PR. |

### Estado tras la unificación del 2026-10-02

Los tres errores que la tabla anterior documenta como deuda preexistente estan
corregidos (items 51 y 55). La tabla se conserva: describe lo que se vio, que es
lo que hace el CI observable, y no lo que se cree hoy.

| Check | Antes (run `36963179520`) | Causa | Ahora |
|-------|--------------------------|-------|-------|
| `workspace-clippy` | failure | `clippy::new_without_default` en `RepairModel` | corregido (item 55) |
| `worker-check` | failure | lifetime en `model.rs` + import sin usar | corregido (item 51) |
| `worker-test` | failure | el mismo error de compilacion | corregid
o (item 51) |
| `worker-clippy` | failure | los dos anteriores con `-D warnings` | corregido (item 51) |
| `workspace-test` | success | — | sin cambios |
| `workspace-fmt` | failure (advisory) | deuda de formato (item 34) | **sigue en rojo a proposito** |

`workspace-fmt` continua siendo advisory y no se ha tocado: activarlo exige
`cargo fmt --all` sobre todo el arbol, que no se puede hacer sin toolchain local
y que es una PR de limpieza con su propia revision del diff.

### Checks del PR #13 (run `36972303464` y `36972303364`)

| Check | Resultado |
|-------|-----------|
| `workspace-test` | pass |
| `workspace-clippy` | pass |
| `worker-check` | pass (host y `wasm32-unknown-unknown --release`) |
| `worker-test` | pass |
| `worker-clippy` | pass |
| `workspace-fmt` | failure, **advisory** por diseno (item 34) |
| `verify` (consistency) | pass, 45 claims, 0 FAIL |
| `validate` (repair-validation) | pass |
| `Cargo Audit` | pass, raiz y worker/, sin advisories |
| `Gitleaks` | pass |
| `Guards de deploy` | pass |
| `build` (wasm.yml) | pass |
| `Workers Builds` | failure **preexistente** (item 56) |
| `Kilo Code Review` | failure **externo** (item 57) |

Los seis checks de `ci.yml` que fallaban en `main` pasan. Los dos unicos rojos
que quedan no son de codigo y estan registrados como tales.

## Auditoria a profundidad 2026-10-03 (leer cada archivo + reparar)

Auditoria completa (todas las ramas, cada carpeta y archivo, compilar/test/clippy
en host y `wasm32-unknown-unknown`): el codigo compilaba y pasaba tests, pero la
auditoria de semantica encontro errores reales y code muerto en `worker/`.
Corregido y push a `main` (`7dd891f`).

| # | Elemento | Estado encontrado | Decision / correccion | Justificacion |
|---|----------|-------------------|-----------------------|---------------|
| 58 | 2 de las 4 señales anti-loop muertas | `same_fingerprint_recent` y `same_failing_verification_recent` hardcodeadas a 0 en `/ingest` y `/attempt`; `ANTI_LOOP_MAX_SAME_FINGERPRINT` /
 `ANTI_LOOP_MAX_SAME_FAILING_VERIFICATION` se leían de `[vars]` pero no se enviaban al DO; tabla `fingerprints` escrita pero nunca leída | **Corregido**: las 4 señales ahora son reales en `/ingest` y `/attempt` (huella contra `incidents.last_fingerprint`; verificaciones `blocked`/`poison` de la tabla `verification`); los límites viajan en el query string | PART3 §12 documentaba "alimentadas por /result": era falso. Con esto los 4 topes de `[vars]` son operativos. |
| 59 | Quota y anti-loop medían la PK | `attempts_of_incident` y `same_incident_recent` usaban `COUNT(*)` sobre `incidents` (PK `id`): siempre 0 o 1, nunca alcanzaban el límite (default 3) → checks inalcanzables | **Corregido**: `attempts` reales desde la columna `attempts` (la incrementa `/attempt`); `same_incident_recent` cuenta transiciones del incidente en la ventana | El límite de intentos del ingest solo tenía sentido con el contador real; antes era dead logic. |
| 60 | Hard stop propio del consumidor inalcanzable | `MAX_QUEUE_ATTEMPTS=3` sobre `QueueTask.attempts`: el body del mensaje es inmutable al reintentar, el contador nunca avanzaba, la rama `if attempt_no >= MAX_QUEUE_ATTEMPTS` jamás se ejecutaba (dead code) | **Corregido**: rama y campo eliminados; el tope es `max_retries=3` de la cola → DLQ y el hard stop del DO (`/attempt`); reintento transitorio con delay fijo de 10 s | El corte por número de intentos ya existía en el DO; el del consumidor era teatro. Documentado en PART3 §10. |
| 61 | UPDATEs por correlación sin rowcount | Un task obsoleto (correlación reemplazada por una entrega nueva) no encontraba su fila: `attempts=0+1=1` → `allowed: true`, los UPDATEs afectaban 0 filas "con éxito" y `/result` sobrescribía `verification` del incidente vigente | **Corregido**: guard `correlation_stale_or_missing` en `/attempt`, `/result` y `/poison`; denegado sin escribir | Single-threaded DO no protege contra tareas que se cruzan entre entregas del mismo incidente. |
| 62 | `from_state` hardcodeado 
| Las transiciones `/result` y `/poison` registraban `from_state='repairing'` incluso cuando el incidente estaba en `queued` (p. ej. compensación `queue_send_failed`) | **Corregido**: `from_state` leído de la fila | Auditoría precisa (PART3 §8). |
| 63 | Idempotencia sin TTL y sin retención | CONTRACT.md §5 declaraba TTL 24 h; `IDEMPOTENCY_TTL_SECONDS` existía en `repair_types` pero nadie la usaba; `idempotency`/`signature_events`/`fingerprints`/`repair_events` crecían sin bound | **Corregido**: `/ingest` aplica el TTL (decisión expirada → reprocesa) y purga: idempotency 24 h, eventos 7 días; `transitions` (auditoría) no se purga | Sin retención el DO crece sin bound (1 GB/objeto Free). |
| 64 | Dos fórmulas de idempotencia | El worker usaba `FNV(delivery_id\|signature)`; el contrato documentaba (y `compute_idempotency_key` calculaba) `FNV(repo\|incident\|delivery\|fingerprint)` — la función canónica estaba sin usar | **Corregido**: el worker usa `repair_types::compute_idempotency_key`; una sola fuente de verdad | Equivalente en dedup (el delivery_id sigue incluido) y consistente con CONTRACT.md §5. |
| 65 | `record_poison` hardcodeaba `DLQ_PROD` | Un veneno de la cola de staging se registraba en el DO como `dlq:auto-healing-repairs-dlq` | **Corregido**: el nombre de cola real (prod/staging) llega al DO | Observabilidad correcta por entorno. |
| 66 | Docs desfasadas de la conducta real | GOVERNANCE (orden de fallback de modelo invertida + "red de ceros"), E2E_CHECKLIST ("cae a red de ceros", "KV sin enlazar", "devuelve PipelineReport"), ARCHITECTURE ("skeleton; NN wiring pending"), PHASE_STATUS ("NN sin cablear"), CONTRACT.md §2 (payload entrante documentado como `RepairEvent` cuando el wire vive en `WebhookPayload`) | **Corregido** en 2026-10-03 | El wire real es `WebhookPayload` (`queue_consumer.rs`); `RepairEvent` queda como contrato versionado objetivo (P1). `verify_repo.py` además ahora cubre `deploy-staging.yml` (antes no chequeaba su `permissions:`). |
| 67 |
 `set-github-secrets.sh` no parseaba | Operador de heredoc escrito `<<<<` (2 usos): bash falla con "Expected redirection target"; el script era inejecutable desde que se versionó | **Corregido** en 2026-10-03: herestring `<<< "$KEY_JSON"` (JSON por stdin a `python3`), que era la intención original | `verify_repo.py` solo comprobaba su existencia (FILES_PRESENT), nunca lo ejecutó: por eso el rojo nunca se vio. |
| 68 | Residuo de comentarios/docs tras la auditoría del 2026-10-03 | `incident_state.rs`: dos pasos "// 4." en `/ingest` (el alta es el 5.º); `lib.rs`: comentario citaba `worker/queue_consumer.rs` (real: `worker/src/worker/`); `deploy.yml`/`deploy-staging.yml`: comentario citaba un `[build]` command que ya no existe (`bash ./build.sh` desde el PR #14); typos: `ci.yml` ("exclia"), `consistency.yml` ("behaves"), `security.yml` ("aparecem", "passesaria"), `INDEX.md` ("PART3async", "Qué guarantee", "todavia"); `INVENTORY.md` ("25 ítems" → 66; "worker esqueleto" → runtime PART3) | **Corregido** en 2026-10-03 en la rama efímera de auditoría | `verify_repo.py` conserva sus propios typos de comentario: es 100755 y el push del conector no puede garantizar el bit, se deja intacto. |

| 69 | Docs desfasadas tras los merges del gap de pesos (2026-10-03) | PART2 ("no hay trainer", "λ sin fijar"), INDEX ("model/ → placeholders; no hay entrenamiento"), INVENTORY ítem 41 ("wrangler.toml conserva REPLACE_WITH" — ya reemplazado por el id real), BRANCH_POLICY/DISCREPANCIES ("origin solo tiene main" — los PRs #18–#20 dejaron residuales), PART1/PROMPT_PAD (sin `repair_train` en el layout), ARCHITECTURE (training path vía `current.json`), REFERENCES (Burn como trainer real) | **Corregido** el 2026-10-03 (PR de docs sync tras auditoría doc↔código) | `verify_repo.py` solo comprueba existencia de estos docs, no su contenido: por eso la deriva no puso ningún claim en rojo |
| 70 | `verify_repo.py` sin los workflows del PR #20 | `WORKFLOWS` no incluía `promote-model.yml` ni `cleanup-b
ranches.yml` → `WORKFLOW_LEAST_PRIVILEGE` no los escaneaba | **Corregido**: añadidos (ambos declaran `permissions:` explícitos) | Mismo patrón que `deploy-staging.yml` antes del PR de auditoría |
| 71 | GATE→PR sin generador de diff (CONTRACT §3, P2) | El contrato exige diff unificado por archivo + apertura de PR (§3–§5); ninguna pieza del repo lo generaba | **Corregido como crate** 2026-10-03 (`crates/repair_pr`: `similar` 3 + `octocrab` 0.54, fail-closed, bin `repair-pr`) | Falta cableado del worker y dispatch por workflow; `attach_bundle` es el único desbloqueo del contrato |
| 72 | Python en un repo Rust y sin vigilancia de deps huérfanas (análisis 2026-10-03, PYH-34) | `verify_repo.py` es el único .py del repo; nada en CI detectaba dependencias declaradas sin uso ni archivos sin enlazar | **Justificado + corregido** 2026-10-03: py = verificador independiente fail-closed (corre aunque el código Rust no compile; no es código de producción); job `unused-deps` en ci.yml (cargo-machete 0.9.2 + cargo-shear 1.14.0, verificados contra docs.rs) + claims `NO_ORPHAN_CRATES`/`NO_ORPHAN_DEPS_CI` | Migrar py a xtask Rust = decisión del dueño; machete puede dar falsos positivos con macros (iterar) |

Ramas: la auditoria dejó el repo en una sola línea, pero los PRs #18–#20
(2026-10-03, tarde) dejaron de nuevo ramas residuales efímeras 100% fusionadas
(`feat/nn-train-v1`, `fix/deep-audit-2026-10-03`, `-v2`, `devops/promote-model-kv`):
`BRANCH_DRIFT` las reporta hasta que `cleanup-branches.yml` las borre bajo
dispatch humano.
