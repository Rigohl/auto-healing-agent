# Auto-Healing Agent

Agente autónomo de auto-reparación (serverless, ~$0/mes).

## Flujo
1. Fallo en **Vercel** o label **`auto-repair`** en **Linear**
2. **Cloudflare Worker** (`worker.js`) → `repository_dispatch`
3. **GitHub Actions** (`.github/workflows/auto-repair.yml`) ejecuta `agent.ts`
4. HF + Exa + MongoDB → rama `fix/auto-repair-*` + PR + comentario Linear + post-mortem

## Archivos (ya en el repo)
| Archivo | Rol |
|---------|-----|
| `worker.js` | Gateway webhooks |
| `.github/workflows/auto-repair.yml` | Orquestador |
| `agent.ts` | Cerebro del agente |
| `mongodb-setup.sh` | Índices Mongo |
| `SECRETS.md` | 9 secrets GitHub |
| `CONFIG.md` | IDs Vercel + Linear |

## Linear (automatizado)
- Team: **Pyh entretainment**
- Label: **`auto-repair`** (existe)
- `LINEAR_API_KEY`: crear en Linear → Personal API keys → secret de GitHub

## Vercel (IDs listos)
- `VERCEL_ORG_ID` = `team_9ILQS3IIe8K4jzv23DSrID4U`
- `VERCEL_PROJECT_ID` ejemplo = `prj_GIuo1hR7R6HKmvSAki93tol4pJrs` (pyr-site-g6jo)
- `VERCEL_TOKEN`: crear en vercel.com/account/tokens

## Solo tú (no automatizable por API)
1. Pegar los **9 secrets** en GitHub Actions
2. Crear Worker en Cloudflare, `wrangler secret put GH_PAT`, desplegar `worker.js`
3. Webhooks Vercel + Linear → URL del Worker

## Prueba rápida (sin Worker)
Repo → Actions → **Auto-Repair Agent** → Run workflow
