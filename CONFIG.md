# Configuración Vercel + Linear (Auto-Healing Agent)

## Vercel (listo para secrets de GitHub)

| Secret | Valor |
|--------|--------|
| `VERCEL_ORG_ID` | `team_9ILQS3IIe8K4jzv23DSrID4U` |
| `VERCEL_PROJECT_ID` | Elige el proyecto a monitorear (ej. `prj_GIuo1hR7R6HKmvSAki93tol4pJrs` = pyr-site-g6jo) |
| `VERCEL_TOKEN` | Crear en https://vercel.com/account/tokens |

### Proyectos detectados (primeros)
- `prj_GIuo1hR7R6HKmvSAki93tol4pJrs` — pyr-site-g6jo
- `prj_CnZbF8abm6FcrDSE1TUM8cM7TNqz` — express-js-on-vercel
- `prj_CYrnnQKIeZkqiraO10OmlxQudfe8` — rigohl-pyh-entertainment-gclh
- `prj_CIYFo9u9L2Tn2kvn8e3bYP0Fpbsq` — vite-react
- `prj_rMofaTXLTtHX6maQyztc7mEBgOui` — j-vairyx-q-0-ui

### Webhook Vercel (después de desplegar el Worker)
1. Vercel → Project → Settings → Webhooks (o Account → Webhooks)
2. URL: `https://TU-WORKER.workers.dev`
3. Eventos: `deployment.error`, `deployment.failed` (o equivalentes de build fallido)
4. Guardar el secret de firma si lo muestra

## Linear (configurado)

| Item | Valor |
|------|--------|
| Team | **Pyh entretainment** (`PYH`) — id `a5fa77d6-935d-4ad6-bb24-197cbaf35a3b` |
| Project | Nuclear Crawler Hybrid — Seguridad e Informática |
| Label | **`auto-repair`** (creado) — al aplicarlo al ticket, el agente se dispara |
| `LINEAR_API_KEY` | Crear en Linear → Settings → Security & access → Personal API keys |

### Webhook Linear (después del Worker)
1. Linear → Settings → API → Webhooks
2. URL: misma del Cloudflare Worker
3. Eventos: Issue (update) / Label
4. Filtrar en el Worker por label `auto-repair`

## Cómo probar Linear sin Worker aún
1. Crear o abrir un issue en team PYH
2. Añadir label **auto-repair**
3. Cuando el Worker + workflow existan, eso disparará el flujo
