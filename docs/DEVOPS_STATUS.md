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
| `73014a1b32b7446397461a8d438c8ab2` | **MODEL_KV** | Referenciado en wrangler.toml (produccion y staging) y promote-model.yml. OK |
| `1dc394e34570414085eb85586ec14912` | STATE | **Orphan**: sin referencia en wrangler.toml ni workflows |
| `3c269e92364f4b59aa83c4388596eefb` | neural-net-weights | **Orphan**: idem |
| `56b993b07d3c4c02903eca621271979f` | CACHE | **Orphan**: idem |
| `9b0fcb8a57e540c08d92ada83c752465` | agent-config | **Orphan**: idem |

**Hallazgo nuevo:** 4 de los 5 namespaces de la cuenta NO estan referenciados por el repo. Decision pendiente del dueno: eliminarlos (reducen superficie) o documentar su proposito.

### Queues y Durable Objects

El wrangler.toml declara 4 colas (produccion/staging x cola principal/DLQ) y 1 clase DO (`IncidentState`). Cloudflare auto-provisiona Queues al deploy (doc oficial); el paso manual "crear colas antes del deploy" de PART3 §17.1/§20.2 esta OBSOLETO (ver DEP-02 / PYH-43).

## Backlog de DevOps (Linear, equipo Pyh entretainment)

Los hallazgos P1-P3 de la auditoria estan registrados como issues PYH-36 a PYH-48, enlazados al PR #32.

## Historial de este documento

- 2026-10-05: creado con estado verificado en vivo (GitHub + Cloudflare). Corrige "DO sin uso" en REFERENCES/PHASE_STATUS/INVENTORY/ARCHITECTURE.

## 2026-10-05 — Reparación integral de main (PR #56)

- **Root cause**: 812f3bb truncó `incident_state.rs` a mitad de `result()`; 441705f introdujo fmt-unclean y `param_derive.rs` sin CI verificado. Último verde: 18d6ca0.
- **Fix**: revert byte-exacto a 18d6ca0 + P2 (alarma DO de retención 24 h, `do_state`) + P3 (`head_sampling_rate = 1`) + docs/WORKERS_BEST_PRACTICES.md. PRs #53/#54/#55 consolidados y cerrados.
- **Pendiente**: re-land de `param_derive.rs` formateado y con CI verde (issue de follow-up); secret WORKER_URL para cerrar SMOKE_WORKER_URL (backlog P4).
- **Ramas**: feat/p2-do-alarm-retention, feat/p3-observability-sampling y docs/workers-best-practices-2026-10-05 marcadas superseded en cleanup-branches (contenido ya en main byte-exacto).
