# GOVERNANCE

```
AUTO_MERGE=false
AUTO_DEPLOY=false
PRODUCTION_WRITE=false
HIGH_RISK_REPAIR=BLOCK
MIN_CONFIDENCE=0.55
MAX_RISK=0.45
```

**Fuente de verdad de umbrales**: este archivo (0.55 / 0.45).

Cadena de autoridad:
```
POLICY → PATCH VALIDATION → CI → VERIFY → PUSH AUTHORIZATION → main
```

## Lógica de gate (resumen)

```
if confidence < MIN_CONFIDENCE → BLOCK (LowConfidence)
if risk > MAX_RISK             → BLOCK (HighRisk)
if operator_id desconocido     → BLOCK
else                           → ALLOW
```

La NN solo propone. Governance + CI/VERIFY deciden.

Notas:
- Valores hot-reloadables vía KV en el futuro.
- Cada decisión se registra en RepairCase para auditoría.
- Por qué no existe fallback a LLM: `docs/NO_LLM_POLICY.md`.
- Qué está verde hoy en el flujo: `docs/E2E_CHECKLIST.md`.
- Documentación expandida de gate/metrics se mantuvo deliberadamente corta; ver DISCREPANCIES si hay conflicto con diseños previos (0.80/0.25).

## Autoridad de VERIFY (auditado 2026-10-02)

GitHub Actions es la autoridad de VERIFY, concretamente los jobs de
`.github/workflows/ci.yml`, que se ejecutan en cada push a `main` y en cada PR:

| Check | Comando | Required propuesto |
|-------|---------|---------------------|
| `workspace-test` | `cargo test --workspace` | si |
| `workspace-clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | si |
| `workspace-fmt` | `cargo fmt --all -- --check` | no aun: advisory hasta limpiar la deuda de formato (item 34 de DISCREPANCIES) |
| `worker-check` | `cargo check --manifest-path worker/Cargo.toml --all-targets` y `--target wasm32-unknown-unknown --release` | si |
| `worker-test` | `cargo test --manifest-path worker/Cargo.toml` | si |
| `worker-clippy` | `cargo clippy --manifest-path worker/Cargo.toml --all-targets -- -D warnings` | si |

`worker/` tiene su propio `[workspace]` (item 35): ningun comando
`--workspace`/`--all` del manifiesto raiz lo compila. Por eso existen los jobs
`worker-*` con `--manifest-path worker/Cargo.toml`.

## Merge (verificado 2026-10-02)

- `AUTO_MERGE=false` (este archivo) es la fuente de verdad: ningun merge sin
  accion humana.
- Mergify NO esta configurado (no existe `.mergify.yml`); no analizar ni
  documentar como si existiera. No se introduce.
- Branch protection de `main`: UNVERIFIABLE/ausente. Evidencia (2026-10-02):
  la API `branches/main/protection` responde 401 sin token admin;
  `rulesets` devuelve `[]`; `main.protected = false`. En consecuencia NINGUN
  check es hoy obligatorio a nivel de plataforma y este documento no afirma
  que GitHub los exija: la exigencia es disciplinaria hasta que una persona
  con permisos de admin los active en Settings -> Branches -> Require status
  checks (los seis nombres exactos de la tabla de arriba).
- Ausentes por decision explicita (crear solo con pedido humano):
  `CODEOWNERS`, `dependabot.yml`, `SECURITY.md`.

## Secret legacy

- `legacy/CONFIG.md` contiene un `VERCEL_ORG_ID` literal (id de equipo de la
  era V0, archivada y no ejecutada). No se rota ni se elimina en silencio:
  decision humana. Ver "Reconciliacion P1" en DISCREPANCIES.
