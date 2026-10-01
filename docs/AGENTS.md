# 14 agentes lógicos (módulos, no LLM, sin Pony)

Cada uno: misión única, I/O, tools, límites, timeout, evidencia, éxito/bloqueo, idempotency, audit.

| # | Agente | Output |
|---|--------|--------|
| 1 | Incident | Incident |
| 2 | Evidence | logs/stack |
| 3 | Repository Analyst | AST/CFG/DFG summary |
| 4 | Failure Signature | FailureSignature |
| 5 | Localization | node_id[] |
| 6 | Neural Repair Policy | RepairAction |
| 7 | Patch Operator | CandidatePatch |
| 8 | Verification | CI PASS/FAIL |
| 9 | Review | notes |
| 10 | Learning | TrainingExample |
| 11 | Training | checkpoint offline |
| 12 | Model Registry | CURRENT/STABLE |
| 13 | Governance | allow/deny |
| 14 | DevOps | PR coord (no override CI) |

Ninguno salta: CI, risk gates, patch limits, protected files, SoT, rollback.

**Pony / actor runtime nativo: fuera de este repo.**

Los agentes son fases, no procesos LLM: ver `docs/NO_LLM_POLICY.md`
(lista de 8 roles) y `PipelinePhase` en `crates/repair_types/src/lib.rs`.
