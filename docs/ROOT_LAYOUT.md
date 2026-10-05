# Raíz del repo vs Notion

## Notion (canónico)

Raíz limpia: `crates/`, `worker/`, `model/`, `tests/`, `scripts/`, `docs/`, `Cargo.toml`, `README`.

**No** en raíz según Notion: agent.ts, worker.js, CONFIG.md, SECRETS.md.

## Hecho

| Antes (raíz) | Ahora |
|--------------|--------|
| agent.ts, worker.js, CONFIG.md / SECRETS.md, mongodb-setup.sh | Eliminados; `legacy/` (archivo V0, no ejecutable) eliminado del repo el 2026-10-05 por decisión del dueño |

Raíz visible en móvil debe priorizar **crates** y **worker**.
