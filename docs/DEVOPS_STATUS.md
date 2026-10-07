# DEVOPS STATUS — Estado verificado en vivo (2026-10-05)

Fuente de verdad de este documento: conectores **GitHub** y **Cloudflare** consultados en vivo el 2026-10-05. Corrige el error de 5 documentos que declaraban el Durable Object "sin uso en el codigo" (era falso: `incident_state.rs` es la pieza transaccional central).

## GitHub — ramas y PRs

| Rama | HEAD | Notas |
|---|---|---|
| `main` | `f123f12` | rama por defecto, sin branch protection |
| `feat/declarative-rules` | `a883519` | PR #31 abierto: reglas declarativas DSL fail-closed (5 commits) |
| `fix/deep-audit-p0-2026-10-05` | `4250d8e` | PR #32 abierto: correcciones P0 de la auditoria (BUG-01/02/03/05/07 + job CI worker-build-artifact) |

PRs abiertos: **#31** (declarative rules) y **#32** (P0 audit fixes).

## CI/CD — GitHub Actions

- Actions activo: **543 workflow runs** historicos en el repo.
- El PR #32 disparo 5 workflows: Consistency (10s), WASM (22s), Security (27s), Repair Validation (1m), CI (2m47s).
- Workflows en el repo: CI, Consistency, Security, WASM, Repair Validation, Regression, Deploy, Deploy Staging, Promote Model, Cleanup Branches, Auto Repair.
- CI es la autoridad de VERIFY (GOVERNANCE.md). Branch protection NO exige los checks todavia (disciplinario).

## Cloudflare — inventario real de la cuenta (verificado en vivo)

### Workers (1)

| Worker | Script ID | Creado | Modificado |
|---|---|---|---|
| `auto-healing-agent` | `7c6b8ec9534b4e97bafa2e2fa79de066` | 2026-09-30 | 2026-10-04 |

Nota: PART3 §23.1 verifico que el script desplegado sigue siendo un placeholder "Hello world" de ~275 bytes. El Worker real (con DO + Queues + KV) **nunca ha sido desplegado**; el camino build.sh + worker-build --panic-unwind se verifica por primera vez en CI con el job nuevo `worker-build-artifact` (PR #32).

### KV namespaces (5)

| Namespace ID | Titulo | Estado en el repo |
|---|---|---|
| `73014a1b32b7446397461a8d438c8ab2` | **MODEL_KV** | Referenciado en wrangler.toml (produccion y staging) y pr
omote-model.yml. OK |
| `1dc394e34570414085eb85586ec14912` | STATE | **Orphan**: sin referencia en wrangler.toml ni workflows |
| `3c269e92364f4b59aa83c4388596eefb` | neural-net-weights | **Orphan**: idem |
| `56b993b07d3c4c02903eca621271979f` | CACHE | **Orphan**: idem |
| `9b0fcb8a57e540c08d92ada83c752465` | agent-config | **Orphan**: idem |

**Hallazgo nuevo:** 4 de los 5 namespaces de la cuenta NO estan referenciados por el repo. Decision pendiente del dueno: eliminarlos (reducen superficie) o documentar su proposito.

### Queues y Durable Objects

El wrangler.toml declara 4 colas (produccion/staging x cola principal/DLQ) y 1 clase DO (`IncidentState`). Cloudflare auto-provisiona Queues al deploy (doc oficial); el paso manual "crear colas antes del deploy" de PART3 §17.1/§20.2 esta OBSOLETO (ver DEP-02 / PYH-43).

## Backlog de DevOps (Linear, equipo Pyh entretainment)

Los hallazgos P1-P3 de la auditoria estan registrados como issues PYH-36 a PYH-48, enlazados al PR #32.
## 2026-10-06 — Configuración exacta de CI/CD (QUÉ DEBE IR EN CADA SITIO)

### A) Cloudflare Dashboard — Workers Builds (Settings > Build)

| Campo | Valor EXACTO | Por qué |
|---|---|---|
| Root directory | `worker` | OBLIGATORIO. Con raíz `/`, wrangler no encuentra wrangler.toml con bindings y desplegaría un Worker sin KV/DO/Queues (commit `2ad38a8` eliminó wrangler.toml/build.sh de raíz por exactamente ese bug) |
| Build command | `bash build.sh` | Workers Builds NO honra el `[build]` del wrangler.toml (doc oficial); build.sh auto-instala rustup si falta y corre `worker-build --release --panic-unwind` |
| Deploy command | `npx wrangler deploy` | Deploy de producción (entorno por defecto). Staging: `npx wrangler deploy --env staging` |
| Preview command | (opcional) `npx wrangler versions upload` | Previews de PRs sin tocar producción |
| API token | el de la cuenta (Cloudflare ya lo tiene si Builds está conectado) | Autentica build+upload |
| Build variables | ninguna necesaria | El build no requiere secrets; los del runtime van en Settings > Variables & Secrets |

Fuente: developers.cloudflare.com/workers/ci-cd/builds/configuration/

### B) Cloudflare Dashboard — Variables & Secrets del Worker (Settings > Variables & Secrets)

| Nombre | Tipo | Uso | Sin él |
|---|---|---|---|
| `WEBHOOK_SECRET` | Secret | fail-closed de `POST /webhook` y `POST /github/callback` (comparación en tiempo constante) | 503 en ambos endpoints |
| `GITHUB_TOKEN` | Secret | `github_client.rs`: rama + commit + PR (permisos mínimos: `contents:write`, `pull_requests:write` sobre el repo objetivo) | todo incidente queda `blocked` con `github_token_not_configured` |
| `MEM0_API_KEY` | Secret (FUTURO, FASE 11) | capa opcional de similitud vía `worker::Fetch` | no-op con log (nunca bloquea) |

Vars de entorno ya declaradas en wrangler.toml (`[vars]`): MONITOR_REPOS, REPAIR_RULES, QUOTA_*, ANTI_LOOP_* — no requieren dashboard.

### C) GitHub Actions — secrets y vars del repo (Settings > Secrets and variables > Actions)

| Nombre | Dónde | Consumido por | Sin él |
|---|---|---|---|
| `CLOUDFLARE_API_TOKEN` | Secret | deploy.yml / deploy-staging.yml (preflight falla) | deploy.yml hace ::error y aborta |
| `CLOUDFLARE_ACCOUNT_ID` | Secret | idem | idem |
| `WEBHOOK_SECRET` | Secret | deploy.yml smoke test (curl a /webhook) | smoke test se omite con notice |
| `WORKER_URL` | Variable (no secret) | smoke test; cierra el claim SMOKE_WORKER_URL de verify_repo.py | smoke se omite; claim queda UNKNOWN (backlog P4) |

deploy.yml NO corre en push (GOVERNANCE.md: AUTO_DEPLOY=false): solo `workflow_dispatch` vía environment `production` con required reviewers.

### D) Inventario Cloudflare verificado en vivo (2026-10-06, actualiza la tabla del 2026-10-05)

KV namespaces: **6** (los 5 anteriores + `996211a015f14c54b85ea4b47e79fdf9` **REPAIR_CASES_KV**, ya referenciado en worker/wrangler.toml prod+staging; el hallazgo "4 orphans" del 2026-10-05 sigue en pie para STATE/neural-net-weights/CACHE/agent-config).
Worker `auto-healing-agent` modificado 2026-10-05T18:03Z; sigue siendo placeholder → el deploy real es el paso humano clave (Linear PYH-61).
Ramas: solo `main` (cleanup-branches auto-deleta las fusionadas).

### E) Qué NO debe existir (por diseño)

- ⚠️ ACTUALIZADO (2026-10-06, decisión del dueño): `wrangler.toml` en la RAÍZ ahora EXISTE como espejo completo de `worker/wrangler.toml` (build cwd=worker, main=worker/build/...) para que Workers Builds con Root directory = / despliegue con TODOS los bindings usando `npx wrangler deploy` tal cual. `validate-preflight.sh` (guard 6) exige equivalencia de claves críticas; si divergen, CI falla. `build.sh` sigue PROHIBIDO en la raíz (solo en worker/).
- ❌ Branch protection que exija checks: pendiente decisión del dueño (hoy es disciplinario).
- ❌ MongoDB/R2: fuera del plan Free; el estado vive en DO+KV.

### F) Falla real de Workers Builds (2026-10-06 19:08 UTC) — firma y fix

Log: `Cloning repository... -> Executing user deploy command: npx wrangler deploy -> ERROR Could not detect a directory containing static files`.
Causa: el deploy corrió en la RAIZ del repo y sin Build command; en la raiz no
hay wrangler.toml (eliminado a proposito, commit 2ad38a8), asi que wrangler
4.148 asumio proyecto de static assets. NO falta ningun archivo del repo: falta
la configuracion del dashboard (Settings > Build):
  Root directory:  worker
  Build command:   bash build.sh
  Deploy command:  npx wrangler deploy
Alternativa con Root directory = raiz: Deploy command
`cd worker && bash build.sh && npx wrangler deploy`. NUNCA recrear
wrangler.toml/build.sh en la raiz (deploy sin bindings). Firmas de esta
familia de errores: (a) "No build output detected to cache" + deploy sin
build previo = falta Build command o Root directory mal; (b) "Could not
detect a directory containing static files" = wrangler sin config en CWD.
Runbook Notion actualizado con esta firma.

- 2026-10-06 (noche): decisión del dueño tras dos fallos reales de Workers Builds ("static files"): wrangler.toml raíz espejo completo (commit 98076b6) + guard 6 de equivalencia en validate-preflight.sh (e7cd2713) + docs alineadas. El deploy desde raíz con `npx wrangler deploy` ya funciona SIN tocar el dashboard.
- 2026-10-07: commit de trigger para forzar un build NUEVO de Workers Builds sobre el main actual (los "Retry" re-ejecutan el snapshot del commit viejo, pre-wrangler.toml raíz). Si este build sigue con "static files", la conexión Git del dashboard apunta a otra rama/repo: verificar Worker > Settings > Build > Git connection (debe ser Rigohl/auto-healing-agent, rama main, Root directory /).

- 2026-10-07 00:38 UTC: build caido con la misma firma "static files". Diagnostico: RETRY de un snapshot PRE-98076b6, no un build del main actual. Evidencia: (a) main actual tiene wrangler.toml raiz valido (guard 6 verde, CI 9/9 en 399f846) y wrangler 4.148 lo habria usado; (b) el log no ejecuto ningun Build command y clono un arbol sin wrangler.toml raiz → snapshot viejo (los Retry re-ejecutan el commit del build original, changelog 2025-03-17). Verificacion decisiva (humano, dashboard): el SHA del commit que lista ese build. Accion: NO volver a Retry del build viejo; disparar un build NUEVO ("Run build" sobre main) o dejar que un push lo dispare.
  Notas verificadas hoy contra docs de Cloudflare: (a) la imagen de Workers Builds NO incluye Rust/cargo (solo Node, Python, Go, Ruby, Bun, etc. — build-image, actualizada 2026-07-30); worker/build.sh YA auto-instala rustup minimal si falta cargo, y rustup instala el toolchain de rust-toolchain.toml. (b) `npx wrangler deploy` SI ejecuta [build] del wrangler.toml (custom-builds docs) aunque Workers Builds como Build step NO lo honra; con el dashboard sin Build command, el build Rust ocurre dentro del deploy command via [build] cwd=worker. Primer build esperado: largo (rustup + toolchain + cargo install worker-build + nightly para --panic-unwind); no confundir timeout con error de config.

## Build 02:15 UTC del 2026-10-07 (post-ddcb3619) — falla RÁPIDA, log pendiente (humano)

- Push `ddcb3619` (docs/INVENTORY.md: inventario profundo 2026-10-07 + corrección del
  falso positivo de tests del contrato) → CI / Consistency ×2 / Security ×2 / Branch
  cleanup TODO VERDE sobre ese commit (15/15 checks).
- El check "Workers Builds: auto-healing-agent" del commit corrió y FALLÓ a los ~5 min
  del push (build `77aeb795-32b6-4c6e-8870-7226d578a86d`, producción). Es el PRIMER
  build disparado sobre un árbol con `wrangler.toml` raíz: ya NO puede ser el error
  "Could not detect a directory containing static files" (eso ocurría sin config en el
  CWD; hoy existe).
- ~5 min es demasiado corto para el build Rust completo (rustup + toolchain +
  worker-build ≈ 10–15 min la primera vez, sin cache) → la falla ocurrió ANTES o al
  inicio del paso de build. Candidatos, en orden:
  (a) validación de esquema de Wrangler sobre el espejo raíz — primera vez que
      wrangler lee esa config en contexto de deploy; tomllib + guard 6 validan
      equivalencia TOML, no el esquema de wrangler;
  (b) fallo temprano de build.sh en la imagen (instalación de rustup/toolchain);
  (c) mecánica skip/superseded (changelog 2026-07-24) — improbable, el check corrió.
- El log SOLO es visible en el dashboard (requiere sesión humana): abrir el build
  `77aeb795` del worker y copiar la primera línea de error — con eso se cierra el
  diagnóstico. No re-retry a ciegas: el snapshot ya es el correcto (ddcb3619).
- Worker sigue placeholder (modified 2026-10-05T18:03Z) → PYH-61 sigue abierto.

## Historial de este documento (actualizado)
- 2026-10-06 (tarde): reparación del bump de actions — el push 0a2dee1 dejó YAML inválido en 7 workflows (deploy-staging, cleanup-branches + 5 más con "Invalid workflow file"). Se reconstruyeron desde 5337044 (último verde) aplicando solo checkout@v5 (commits b109e04, 5b38422, be3ccad, 0a4585a, 04fe0c5, 9c8b6b3, 0b76945). En 0b76945: CI ✅, Consistency ✅ (verify 50 claims), Security ✅, Branch cleanup ✅. Nota técnica: el "content viewer" open_url parte líneas al azar al mostrar YAML — verificación de contenido siempre vía raw fetch (apify web-fetch), nunca open_url.
- 2026-10-06: sección CI/CD completa (dashboard + secrets + vars, valores exactos); inventario KV actualizado a 6 namespaces; referencias a commits que fijaron cada decisión.
- 2026-10-05: creado con estado verificado en vivo (GitHub + Cloudflare).
