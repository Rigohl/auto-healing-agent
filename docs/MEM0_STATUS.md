# Estado de Mem0

> Actualizado 2026-10-06 (verificación en vivo desde el workspace de Vibe).

## Estado actual

| Aspecto | Estado |
|---------|--------|
| Conector MCP en este workspace (Vibe) | ❌ No disponible (verificado 2026-10-06: no aparece en el inventario de conectores) |
| Uso desde el Worker | ❌ Ninguno (por diseño: estado cubierto por DO + KV; ver MEM0_ANALYSIS.md) |
| Claim "workspace connector OPERATIONAL" (2026-10-05) | ⚠️ Registrado en IMPLEMENTATION_PROGRESS.md como verificado ese día con otro cliente; no reproducible desde Vibe el 2026-10-06 |
| "Mem0 simulado" del historial | Los archivos de docs de este repo (MEM0_ANALYSIS/MEM0_STATUS) son la memoria documental, no un servicio |

## Por qué no es crítico para auto-repair
El estado del pipeline ya está cubierto: Durable Object (SQLite) para
dedup/quota/anti-loop, REPAIR_CASES_KV para RepairCase persistente. Mem0
sería una capa OPCIONAL de similitud semántica ("incidentes parecidos"),
nunca fuente de verdad.

## Diseño futuro (FASE 11)
- Tras VERIFY=PASS → persistir RepairCase en Mem0 (async, best-effort) con
  metadata (operator_id, confidence, risk, verify_result) y fallback si cae.
- Desde el Worker: usar worker::Fetch contra la API HTTP de Mem0 con el
  mismo patrón fail-closed de worker/src/worker/github_client.rs
  (secret MEM0_API_KEY; sin él: no-op con log, nunca bloquea la reparación).
- Tools esperadas del MCP cuando exista: mem0_add_memory,
  mem0_search_memory, mem0_list_memories.

## Instalación preparada (MCP)

```bash
npx mcp-add --name mem0-mcp --type http \
  --url "https://mcp.mem0.ai/mcp/" \
  --clients "cursor"
```
