# Dashboard de observabilidad (FASE 1)

`GET /dashboard` del Worker agrega los RepairCases persistidos en
`REPAIR_CASES_KV` (clave `repair_case:{correlation_id}`) y pinta HTML+SVG
generado server-side en Rust. Sin assets JS externos, sin LLM, sin
dependencias nuevas (docs/NO_LLM_POLICY.md). Implementacion:
`worker/src/worker/dashboard.rs` (handler en `worker/src/lib.rs`).

## Autorizacion (fail-closed)

- Secret del Worker `DASHBOARD_TOKEN`:
  `cd worker && npx wrangler secret put DASHBOARD_TOKEN`
- Header obligatorio `x-dashboard-token`.
- Sin secret configurado (o vacio): 503 `dashboard_token_not_configured`.
- Token incorrecto: 401 `unauthorized` (comparacion en tiempo constante,
  `runtime::security::constant_time_eq`).
- El dashboard es SOLO lectura: nunca repara, nunca encola, nunca toca
  la autoridad de VERIFY (GitHub Actions).

## Que muestra

- Cards: total de RepairCases, VERIFY pass/fail, pendientes de
  verificacion, PRs abiertos.
- Barras por operador (conteo descendente, empates por nombre).
- Tendencia diaria UTC (ultimos 14 dias).

## Limites conscientes (tier gratis)

- Una pagina de `list` (200 claves) por render; si hay mas claves, la
  pagina lo indica. El dashboard es observabilidad, no un export.
- Los casos con JSON malformado se ignoran (cuentan como ausentes).

## Ejemplo

```bash
curl -H "x-dashboard-token: $DASHBOARD_TOKEN" \
  https://auto-healing-agent.<cuenta>.workers.dev/dashboard
```

## FASE 2 (backlog P2 / PYH-60)

Workers Analytics Engine: el pipeline escribe datapoints por reparacion
(writeDataPoint) y el dashboard consulta la SQL API para series
temporales sin escanear KV. Requiere token con Account Analytics Read.
