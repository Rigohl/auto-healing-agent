# Política de ramas

**Persistente:** solo `main`. Nada más.

## Estado verificado (2026-10-01)

| Comprobación | Resultado |
|--------------|-----------|
| Ramas locales | 1 — `main` |
| Ramas en `origin` | 1 — `refs/heads/main` |
| PRs abiertos | 0 |
| Tags | 0 |

Las 9 ramas que existían al inicio se eliminaron tras extraer su delta útil:

| Rama | Motivo de la eliminación |
|------|--------------------------|
| `setup/secrets`, `feat/rust-wasm-nn`, `fix/safejson-null-payload` | Ancestros lineales de `main`; `git diff origin/main...origin/<rama>` vacío |
| `chore/unify-docs-cleanup`, `chore/enrich-docs-from-kilo` | Contenido ya presente en `main` (PART1–4, INVENTORY, INDEX, MEM0_STATUS, GOVERNANCE); los ítems 18/19 ya estaban |
| `feat/rust-complete-stack`, `docs/prompt-pad-part1-2` | Originales verbosos de PART1–4; los condensados preservan todas las decisiones |
| `feat/rust-nn-core` | Delta útil extraído (`PipelinePhase`, `verify_result`, `NO_LLM_POLICY.md`, `E2E_CHECKLIST.md`); `PONY.md` descartado por `AGENTS.md`; código restante superado por `main` |
| `kilo/bionic-owl-ok9` | Absorbido depurado (`REFERENCES.md`); `DOCUMENTATION.md` y `ACADEMIC_REFS.md` rechazados (ítems 21 y 24) |

Detalle por rama: `docs/INVENTORY.md`.

## Reglas

1. Cualquier rama que no sea `main` es **efímera**: se elimina tras merge o abandono.
2. Nada de ramas `feature/*`, `fix/*`, `docs/*`, `setup/*` ni `kilo/*` persistentes.
3. El trabajo llega a `main` por commits Conventional Commits directamente, o por PR
   efímera que se borra al mergear.
4. Una rama no se considera eliminada hasta que no quedan PRs abiertos ni tags
   que la refieran.

## Nota

Pony (actor runtime nativo) está fuera de alcance y no se menciona como fase del
proyecto: `docs/PHASE_STATUS.md` lo omite y `docs/AGENTS.md` lo excluye.
`docs/DISCREPANCIES.md` ítem 23 conserva el histórico de la decisión.