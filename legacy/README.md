# Legacy (V0 LLM)

Archivo histórico. **No se ejecuta.** Ningún workflow del núcleo Rust invoca
estos archivos; `.github/workflows/auto-repair.yml` está deshabilitado y sólo
imprime un aviso (`workflow_dispatch`, sin ruta HF).

## Clasificación

| Archivo | Estado | Razón |
|---------|--------|-------|
| `agent.ts` | histórico valioso | Documenta la arquitectura V0 (HuggingFace escribiendo ficheros). Referenciado por `docs/DISCREPANCIES.md` ítems 2, 3 y 13, y por el encabezado de `auto-repair.yml`. |
| `worker.js` | histórico valioso | Gateway JS V0. El sustituto vigente es `worker/` (workers-rs). |
| `CONFIG.md` | histórico valioso | Vercel + Linear. **Ver aviso de credenciales abajo.** |
| `SECRETS.md` | histórico valioso | Nombres de secrets de la era LLM/HF. Sin valores. |
| `mongodb-setup.sh` | histórico valioso | Único rastro del esquema MongoDB en el repo. **No hay driver Rust**: MongoDB sigue diferido (`DISCREPANCIES` 10). |

Nada se elimina: ninguno de estos archivos está duplicado en otra ruta y
`agent.ts` / `worker.js` conservan el diseño V0 que `docs/NO_LLM_POLICY.md`
describe como descartado.

## Aviso de credenciales

`CONFIG.md` contiene un valor literal en `VERCEL_ORG_ID`
(`team_9ILQS3IIe8K4jzv23DSrID4U`). Se conserva por valor histórico, pero:

- **No es una credencial vigente.** Se trata como valor revocado / placeholder.
- Si ese org-id llegara a estar activo, debe rotarse en Vercel y eliminarse del
  historial mediante el procedimiento del proveedor.
- Ningún archivo de este directorio debe usarse como fuente de configuración.

## Regla

**No ampliar.** El target es `crates/*` + `worker/`. Cualquier archivo nuevo en
`legacy/` debe justificarse como histórico; no se reintroduce `agent.ts`, HF ni
ninguna ruta LLM al núcleo.