# 14 logical agents (no LLM generation)

| # | Agent | Output |
|---|-------|--------|
| 1 | Incident | Incident |
| 2 | Evidence | evidence bundle |
| 3 | Repository Analyst | AST/CFG/DFG summary |
| 4 | Failure Signature | FailureSignature |
| 5 | Localization | node_id candidates |
| 6 | Neural Repair Policy | RepairAction |
| 7 | Patch Operator | CandidatePatch |
| 8 | Verification | CI PASS/FAIL |
| 9 | Review | review notes |
| 10 | Learning | TrainingExample |
| 11 | Training | checkpoint offline |
| 12 | Model Registry | CURRENT/STABLE |
| 13 | Governance | allow/deny |
| 14 | DevOps | PR coord |

None may skip CI, risk gates, patch limits, protected files, SoT, rollback.
