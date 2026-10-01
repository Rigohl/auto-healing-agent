# BRANCH POLICY & DRIFT CONTROL

```
ACTIVE_BRANCH_POLICY=single_branch (main)
ENFORCE_BRANCH_DRIFT_CHECK=true
```

## Política de Ramas

1. **`main` es la única rama activa de desarrollo e integración.**
2. Todas las modificaciones se introducen mediante Pull Requests aislados que ejecutan la suite completa de CI y verificación de consistencia (`verify-consistency.yml`).
3. No se admiten ramas remotas de larga duración ni acumulaciones de ramas divergentes.

## Estado Real de Ramas Verificado

A fecha de auditoría:
- `main`: Rama de producción principal.
- `remotes/origin/fix/workers-builds-rust-toolchain`: Rama de fix integrada upstream (commit `9e0ae98`).
- `remotes/origin/fix/wrangler-cloudflare-binding`: Rama de fix integrada upstream (commit `5cbbf8c`).

Cualquier PR o rama divergente debe rebasarse linealmente sobre `main` antes de ser evaluada por el gate de CI.
