# REFERENCIAS — enlaces oficiales y glosario

> Absorbido de `kilo/bionic-owl-ok9:docs/07_REFERENCES/{TECH_SOURCES,GLOSSARY}.md`
> (commit `cdada9b`), **depurado** contra el código real de `main`.
>
> Solo se conservan enlaces a documentación oficial y un glosario corregido.
> Ver `docs/DISCREPANCIES.md` ítems 24 y 25 sobre lo que se descartó y por qué.

## Fuentes técnicas oficiales

### Cloudflare Workers

| Tema | URL |
|------|-----|
| Workers Overview | https://developers.cloudflare.com/workers/ |
| Rust en Workers | https://developers.cloudflare.com/workers/languages/rust/ |
| WASM Runtime API | https://developers.cloudflare.com/workers/runtime-apis/webassembly/ |
| Límites de la plataforma | https://developers.cloudflare.com/workers/platform/limits/ |
| Wrangler (config) | https://developers.cloudflare.com/workers/wrangler/configuration/ |
| workers-rs | https://github.com/cloudflare/workers-rs |
| workers-rs (ejemplos) | https://github.com/cloudflare/workers-rs/tree/main/examples |
| Durable Objects | https://developers.cloudflare.com/durable-objects/ |
| DO (límites) | https://developers.cloudflare.com/durable-objects/platform/limits/ |
| R2 (API en Workers) | https://developers.cloudflare.com/r2/api/workers/workers-api-reference/ |
| KV | https://developers.cloudflare.com/kv/ |

Límites Free aplicables ya consolidados en `docs/PART3_CLOUDFLARE_RUNTIME.md`.
No se repiten aquí para evitar dos fuentes de verdad.

### Rust / WASM toolchain

| Tema | URL |
|------|-----|
| `wasm32-unknown-unknown` | https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html |
| wasm-bindgen (guide) | https://rustwasm.github.io/wasm-bindgen/ |
| wasm-bindgen (repo) | https://github.com/wasm-bindgen/wasm-bindgen |
| serde-wasm-bindgen | https://github.com/wasm-bindgen/serde-wasm-bindgen |
| wasm-opt (WABT) | https://github.com/WebAssembly/wabt |
| cargo-bloat | https://github.com/RazrFalcon/cargo-bloat |
| worker-build | https://github.com/cloudflare/workers-rs/
tree/main/worker-build |

Target fijado en `rust-toolchain.toml`: `wasm32-unknown-unknown`, toolchain `stable`.

### Frameworks de red neuronal

| Framework | URL | Nota |
|-----------|-----|------|
| Burn | https://burn.dev/ | Referencia de diseño de training; el trainer V1 real es propio (`crates/repair_train`) |
| Burn (docs.rs) | https://docs.rs/burn/latest/burn/ | |
| Burn (repo) | https://github.com/tracel-ai/burn | |
| Burn (ejemplo MNIST WASM) | https://github.com/tracel-ai/burn/tree/main/examples/mnist-inference-wasm | Referencia WASM |
| burn-autodiff | https://docs.rs/burn-autodiff/latest/burn_autodiff/ | Decorator de autodiff |
| tract | https://github.com/sonos/tract | Alternativa de inferencia, evaluada |

`candle` queda fuera: no es la ruta de entrenamiento del proyecto.

### Persistencia (todavía **no conectadas** en código)

| Servicio | URL |
|----------|-----|
| MongoDB Atlas | https://www.mongodb.com/docs/atlas/ |
| MongoDB Rust driver | https://docs.rs/mongodb/latest/mongodb/ |
| MongoDB connection string | https://www.mongodb.com/docs/manual/reference/connection-string/ |
| Notion API | https://developers.notion.com/reference/intro |
| Mem0 | https://docs.mem0.ai/ |

Enlace ≠ integración. No hay driver Rust de MongoDB, ni conector Mem0, ni bindings
de R2/D1/DO en el árbol: ver `docs/DISCREPANCIES.md` ítems 9, 10 y 21.

### GitHub / CI-CD

| Tema | URL |
|------|-----|
| GitHub Actions | https://docs.github.com/en/actions |
| octocrab (API GitHub en Rust) | https://docs.rs/octocrab/latest/octocrab/ |
| wrangler GitHub Action | https://github.com/cloudflare/wrangler-action |
| cargo-deny | https://github.com/EmbarkStudios/cargo-deny |

### Regla de verificación de API (Context7)

| Herramienta | URL |
|-------------|-----|
| Context7 | https://context7.com/docs/agentic-tools/overview |
| Context7 (repo) | https://github.com/upstash/context7 |

Obligatorio antes de escribir código contra cualquier API/crate externo.
Detalle en `docs/PART4_PERSISTENCE_TRANSVERSAL.md`.

## Glosario (corregido contra el código)

Definiciones alineadas con los tipos reales de `crates/repair_types/src/lib.rs`.
Donde la fuente absorbida discrepaba, se indica.

### Pipeline

| Término | Definición |
|---------|------------|
| **Incident** | Fallo normalizado: `id, source, error_code, error_step, command, message, project, attempts, stack_hint, language_hint, framework_hint, verified, status` |
| **FailureSignature** | `error_code, error_step, command_family, language, framework, fingerprint`. `command_family` ∈ `vercel_build`, `node_package`, `cargo`, `other` |
| **RepairAction** | Propuesta de la NN: `node_id, repair_operator, parameters, confidence, risk`. **Nunca** texto libre |
| **CandidatePatch** | Salida de `repair_operators::apply`: `operator, summary, files, steps, advisory` |
| **RepairCase** | `incident_id, signature, action, verification, patch_summary, pr_url, reward, created_at_unix` |
| **TrainingExample** | `features, operator, node_id, reward, verified` (se genera solo con VERIFY PASS) |
| **OperatorId** | Enum 0–12 (`OPERATOR_COUNT = 13`) + `Unknown = 255`. Allowlist cerrada |
| **FeatureVector** | `[f32; 64]`, `DIM = 64`, `SCHEMA_VERSION = 2` |
| **VerificationResult** | `Pass`, `Fail`, `Blocked`, `Skipped` |

### Red neuronal

| Término | Definición |
|---------|------------|
| **NN core** | `crates/repair_nn_core`: MLP `no_std` + alloc, `64 → 32 → 16` |
| **WEIGHT_COUNT** | **2863** (`crates/repair_nn_core/src/lib.rs:18`) |
| **Operator head** | `16 → 13` logits → softmax → `operator_id` |
| **Confidence head** | `16 → 1` → sigmoid → `confidence ∈ [0,1]` |
| **Risk head** | `16 → 1` → sigmoid → `risk ∈ [0,1]` |
| **Gate** | `confidence ≥ 0.55 ∧ risk ≤ 0.45 ∧ operator != NoOp/Unknown`. SoT: `docs/GOVERNANCE.md` |
| **Latent** | Capa intermedia, 16 dimensiones |

### GATE→PR (V1)

| Tema | URL |
|------|-----|
| similar (diff unificado) | https://crates.io/crates/similar |
| similar (docs.rs) | https://docs.rs/si
milar |
| octocrab (cliente GitHub / PR) | https://crates.io/crates/octocrab |
| octocrab (docs.rs) | https://docs.rs/octocrab |

Versiones ancladas en `crates/repair_pr/Cargo.toml` (verificadas contra crates.io, 2026-10-03): `similar = "3"` (3.2.0), `octocrab = "0.54"` (0.54.2), `secrecy = "0.10"` (0.10.3). Crates std fuera del edge: el Worker nunca abre PRs.

### Código huérfano (deps sin uso / archivos sin enlazar)

| Fuente | Uso en el repo | Verificado |
|--------|----------------|------------|
| cargo-machete 0.9.2 (github.com/bnjbvr/cargo-machete) | job `unused-deps` de ci.yml: dependencias declaradas que ningún crate usa (crates/ + worker/); exit != 0 con hallazgos | docs.rs / crates.io, 2026-10-03 |
| cargo-shear 1.14.0 (github.com/Boshen/cargo-shear) | mismo job: deps del workspace sin usar + advertencias de archivos vacíos/sin enlazar | docs.rs / crates.io, 2026-10-03 |

### Runtime

| Término | Definición |
|---------|------------|
| **Worker** | Cloudflare workers-rs. Orquestador, **no** motor de cómputo |
| **wasm32-unknown-unknown** | Target Rust→WASM estándar. **No** WASI, **no** Emscripten |
| **wasm-bindgen** | Genera bindings JS↔Rust. Aislado en `repair_nn_wasm`; el core no lo depende |
| **wasm-opt -Oz** | Optimizador por tamaño. Relevante para el límite de bundle |
| **no_std** | Rust sin `std` (sólo `core` + `alloc`), requerido para el core y WASM |

### Memoria (diseño, no implementada)

| Término | Definición |
|---------|------------|
| **KV** | Store global de Cloudflare. En el código: solo puntero `model/current` |
| **R2** | Object storage S3-compatible. **Sin binding en `wrangler.toml`** |
| **DO** | Entidad stateful de Cloudflare. **Sin uso en el código** |
| **MongoDB** | Memoria operacional/semántica según diseño. **Sin driver Rust** |
| **Mem0** | Memoria semántica. **Conector pendiente, no operativo** |
| **Notion** | Autoridad de diseño e histórico. **Nunca** autoridad del código |

### Siglas

CF, CI, CD, DO, KV, NN, PR, R2, WASM, AST, HMAC, TTL, ECE, P99, OOM, DLQ.

## Lo que se descartó de la fuente

- **Tabla de versiones** ("Rust 1.82+", `burn 0.16.1`, `wasm-bindgen 0.2.95`, `worker 0.0.12`, `mongodb 3.2`, `octocrab 0.28`…): no verificable en este entorno y atribuida a "Context7 / crates.io" sin cita. `rust-toolchain.toml` fija `stable`, sin versión. No se copia.
- **Columnas "última actualización"** ("Abr 2026", "Jul 2026"): no verificables.
- **Referencias académicas**: ver `docs/DISCREPANCIES.md` ítem 24.