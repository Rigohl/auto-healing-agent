# Fases (Pony omitida · TS/LLM path off)

| Fase | Estado |
|------|--------|
| 1 Inventario | completed |
| 2 Representación | partial (encoder V1: los 64 slots llevan señal, con test de no-constancia; falta señal AST/CFG — `DISCREPANCIES` 25) |
| 3 Repair engine | partial (NN + operadores; GATE→PR V1: generador de diff real `repair_pr` — falta cableado del worker) |
| 4 Verification | partial (ci/wasm; legacy HF off) |
| 5 Learning | partial (pesos V1 entrenados y exportados: `crates/repair_train` + `model/current.txt`, λ=0.5 fijada; falta promoción a KV y bucle online) |
| 6 Cloudflare | completed como runtime (asíncrono PART3: webhook fail-closed → DO → Queue → pipeline; señales anti-loop reales, TTL y retención). Deploy bloqueado por credenciales — `DISCREPANCIES` 41 |
| 7 Agentes docs | completed (`AGENTS.md` + `NO_LLM_POLICY.md`) |
| 8 Pony | **omitted** — actor runtime nativo fuera del repo (`AGENTS.md`) |
| 9 E2E | not green — ver `docs/E2E_CHECKLIST.md` |

Notas de governance:
- `MIN_CONFIDENCE=0.55` / `MAX_RISK=0.45` (`GOVERNANCE.md`, SoT).
- Ninguna fase se declara completada por `confidence` del modelo: la autoridad
  de VERIFY es GitHub Actions (`NO_LLM_POLICY.md`).
- - Mem0, R2 y el driver Rust de MongoDB siguen **planificados y ausentes del codigo**. El **DO SI esta implementado** (clase `IncidentState`, binding `INCIDENT_STATE`, wrangler.toml; ~1.000 lineas en `worker/src/worker/incident_state.rs`): dedup, idempotencia, quota y anti-loop.
  ausentes del código** (`DISCREPANCIES` 9, 10, 21). No se inventan.