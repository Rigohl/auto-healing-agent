# Política de ramas

**Persistente:** solo `main`. Nada más.

## Estado verificado (2026-10-01)

| Comprobación | Resultado |
|--------------|-----------|
| Ramas locales | 1 — `main` |
| Ramas en `origin` | 1 — `refs/heads/main` |
| PRs abiertos | 0 |
| Tags | 0 |

### Estado tras la segunda unificación (verificado 2026-10-02)

Comprobado contra la API de GitHub, no de memoria:

| Comprobación | Resultado |
|--------------|-----------|
| Ramas en `origin` | 2 — `main` y `chore/unify-branches-into-main` (la efímera del PR #13) |
| Ramas en `origin` tras mergear el PR #13 | 1 — `main` |
| PRs abiertos | 1 — el #13; el #11 (DRAFT) se cerró sin merge porque su contenido entra por el #13 |
| Tags | 0 |

Las 9 ramas se borraron con `DELETE /repos/:owner/:repo/git/refs/heads/<rama>`,
después de integrar su delta útil. Ninguna tenía trabajo sin integrar: lo que no
era útil ya estaba en `main` y lo que lo era está ahora.

Esa tabla era cierta el 2026-10-01 y dejo de serlo el 2026-10-02: las PRs #6 a
#12 dejaron 9 ramas. El estado real de hoy lo comprueba `scripts/verify_repo.py`
(claim `BRANCH_DRIFT`) en vez de una tabla escrita a mano, porque una tabla
escrita a mano no se entera de que vuelve a haber ramas.

Las 9 ramas que existían al inicio se eliminaron tras extraer su delta útil:

| Rama | Motivo de la eliminación |
|------|--------------------------|
| `setup/secrets`, `feat/rust-wasm-nn`, `fix/safejson-null-payload` | Ancestros lineales de `main`; `git diff origin/main...origin/<rama>` vacío |
| `chore/unify-docs-cleanup`, `chore/enrich-docs-from-kilo` | Contenido ya presente en `main` (PART1–4, INVENTORY, INDEX, MEM0_STATUS, GOVERNANCE); los ítems 18/19 ya estaban |
| `feat/rust-complete-stack`, `docs/prompt-pad-part1-2` | Originales verbosos de PART1–4; los condensados preservan todas las decisiones |
| `feat/rust-nn-core` | Delta útil extraído (`PipelinePhase`, `verify_result`, `NO_LLM_POLICY.md`, `E2E_CHECKLIST.md`); `PONY.md` descartado por `AGENTS.md`; código restante superado por `main` |
| `kilo/bionic-owl-ok9` | Absorbido depurado (`REFERENCES.md`); `DOCUMENTATION.md` y `ACADEMIC_REFS.md` rechazados (ítems 21 y 24) |

Detalle por rama: `docs/INVENTORY.md`.

## Segunda unificación (2026-10-02): las 9 ramas que reaparecieron

Las PRs #6 a #12 dejaron 9 ramas en `origin`. Ninguna se integró por merge: cada
una nacía de un `main` distinto y varias **revolvían** trabajo ya presente. Se
extrajo el delta útil commit a commit y se borraron las ramas.

| Rama | PR | Decisión |
|------|----|----------|
| `chore/p1-ci-real-governance` | #12 MERGED | Fuera de `main` por 1 commit y 0 por delante: ya estaba. Se borra. |
| `feat/contract-github-cloudflare-4476481194757304259` | #11 DRAFT | **Delta útil**: `repair_types::contract`, sus tests y `docs/CONTRACT.md`. Se integra corregido. |
| `feature-encoder-pipeline-fix-12040107054014113875` | #6 CLOSED | **Delta útil**: encoder V1 + `synthetic.rs`. Su `worker/src/lib.rs` se rechaza: revierte el runtime PART3. |
| `fix/ci-fmt-clippy-workspace-9559682751243179716` | #9 CLOSED | **Delta útil**: `impl Default for RepairModel`. El resto es reformateo y el mismo rollback del worker. |
| `fix/ci-security-governance-reproducible-builds-11007936177750635441` | #10 CLOSED | **Delta útil**: `permissions`, `concurrency`, rust-cache, cargo-audit, gitleaks. Se rechaza su rewrite de `ci.yml` y el pineo de toolchain. |
| `fix/unblock-deploy-reproducible-6498651806722640463` | #8 CLOSED | **Delta útil**: `build.sh` y symlink `wrangler.toml` en raíz, `scripts/validate-preflight.sh`. |
| `fix/workers-builds-rust-toolchain` | #5 MERGED | Idéntica a `main`: `worker/build.sh` byte a byte. Se borra. |
| `fix/wrangler-cloudflare-binding` | #4 MERGED | `main` ya tiene un `wrangler.toml`strictmente superior (DO, colas, staging). Se borra. |
| `jules-5813573718571814256-249ce1c3` | #7 CLOSED | **Delta útil**: matriz de transiciones en GOVERNANCE y `scripts/verify_repo.py`. Se reescribe: sus claims apuntaban a código pre-PART3. |

### Por qué no se hizo merge y se extrajo commit a commit

Un `git merge` de estas ramas habría traído el rollback del runtime async por
detrás: cuatro de ellas modifican `worker/src/lib.rs` para volver al pipeline
síncrono V0, borrando el Durable Object, la Queue, `#[event(queue)]`, las quotas
y el anti-loop, y degradando el chequeo del secret a igualdad simple. El diff de
esas cuatro es en su mayoría código que `main` ya tenía, escrito de otra forma.

El detalle de qué se rechaza y por qué está en el mensaje de cada commit y en
`docs/DISCREPANCIES.md`. La regla que lo hace sostenible es la siguiente.

## Tercera unificación (2026-10-03): limpieza de residuales + una sola línea

Tras la DevOps PR #16, `origin` volvía a tener 3 ramas residuales
(`devops/staging-pipeline`, `fix/root-build-cd-worker`, `fix/worker-build-cwd`):
100% fusionadas en `main` (verificado con `git merge-base --is-ancestor`), pero
aún presentes, y por eso `verify_repo.py` fallaba `BRANCH_DRIFT`.

- **Borradas** el 2026-10-03: `devops/staging-pipeline` (`cb889cf`),
  `fix/root-build-cd-worker` (`e781600`), `fix/worker-build-cwd` (`493594e`).
  Pérdida nula: todos sus commits son alcanzables desde `main`.
- Desde entonces `origin` solo tiene `main`. Estado: **una sola línea**.
- En local se aplicó la misma regla: no quedan ramas efímeras; el trabajo
  llega a `main` directamente (commit `7dd891f`: auditoría a profundidad y
  reparación de errores reales del runtime, ver `DISCREPANCIES` 58–66).

## Reglas

1. Cualquier rama que no sea `main` es **efímera**: se elimina tras merge o abandono.
2. Nada de ramas `feature/*`, `fix/*`, `docs/*`, `setup/*` ni `kilo/*` persistentes.
3. El trabajo llega a `main` por commits Conventional Commits directamente, o por PR
   efímera que se borra al mergear.
4. Una rama no se considera eliminada hasta que no quedan PRs abiertos ni tags
   que la refieran.
5. **Una rama que se abre desde `main` viejo y luego se queda atrás no se mergea.**
   Se leen sus commits y se aplican a `main` solo los que aportan algo. El motivo
   concreto: un merge arrastra el historial entero de la rama, y en estas nueve
   ese historial incluía un rollback del runtime de producción. Un PR que "trae
   el contrato" puede traerse por detrás el Worker al estado previo a PART3.
6. `scripts/verify_repo.py` (claim `BRANCH_DRIFT`, workflow `consistency.yml`)
   comprueba esta política en cada push y PR. La rama bajo revisión se identifica
   por `GITHUB_HEAD_REF`, así que el claim no pasa en verde por comprobarse a sí mismo.

## Nota

Pony (actor runtime nativo) está fuera de alcance y no se menciona como fase del
proyecto: `docs/PHASE_STATUS.md` lo omite y `docs/AGENTS.md` lo excluye.
`docs/DISCREPANCIES.md` ítem 23 conserva el histórico de la decisión.