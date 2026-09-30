# Raíz del repo vs Notion

## Notion (canónico)

Raíz limpia: `crates/`, `worker/`, `model/`, `tests/`, `scripts/`, `docs/`, `Cargo.toml`, `README`.

**No** en raíz según Notion: agent.ts, worker.js, CONFIG.md, SECRETS.md.

## Hecho

| Antes (raíz) | Ahora |
|--------------|--------|
| agent.ts | `legacy/agent.ts` |
| worker.js | `legacy/worker.js` |
| CONFIG.md / SECRETS.md | `legacy/` |
| mongodb-setup.sh | `legacy/` |

Raíz visible en móvil debe priorizar **crates** y **worker**.
