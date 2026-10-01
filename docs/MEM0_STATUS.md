# Estado de Mem0

> Conector **no operativo**. No se finge consulta.

## Estado actual

| Aspecto | Estado |
|---------|--------|
| Conector MCP | ❌ No operativo |
| Instalación preparada | ✅ `npx mcp-add --name mem0-mcp ...` |
| Tools | Ninguna disponible |
| Uso actual | Documentado solo |

## Preparación

```bash
npx mcp-add --name mem0-mcp --type http \
  --url "https://mcp.mem0.ai/mcp/" \
  --clients "cursor"
```

Tools esperadas (cuando exista): `mem0_add_memory`, `mem0_search_memory`, `mem0_list_memories`.

## Diseño futuro (FASE 11)
Tras VERIFY=PASS → persistir RepairCase en MongoDB + (async) Mem0 con metadata (operator_id, confidence, risk, verify_result).

Payload ejemplo:
```json
{
  "content": "Incident syntax_error → DependencyRepair (conf=0.87, risk=0.13) → PASS",
  "metadata": {
    "operator_id": 2,
    "confidence": 0.87,
    "risk": 0.13,
    "verify_result": "PASS"
  },
  "tags": ["auto-healing", "repair-case"]
}
```

## Por qué no ahora
1. Conector no funcional.
2. MongoDB + Notion ya cubren operacional + histórico.
3. YAGNI + honestidad técnica.

## Checklist activación
- [ ] Conector Mem0 operativo
- [ ] `MEM0_API_KEY` en secrets
- [ ] Flag `MEM0_ENABLED`
- [ ] Integración async en persist
- [ ] Fallback si Mem0 caído
