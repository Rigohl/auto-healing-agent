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
