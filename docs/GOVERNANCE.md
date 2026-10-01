# GOVERNANCE & AUTHORITY MODEL

```
AUTO_MERGE=false
AUTO_DEPLOY=false
PRODUCTION_WRITE=false
HIGH_RISK_REPAIR=BLOCK
MIN_CONFIDENCE=0.55
MAX_RISK=0.45
```

**Fuente de verdad de umbrales**: este archivo y `worker/src/lib.rs` (MIN_CONFIDENCE=0.55 / MAX_RISK=0.45).

## Cadena de Autoridad y Matriz de Transiciones

El sistema opera bajo un modelo de **cero autoridad implícita del agente** para merge o deploy.

```
ISSUE → INCIDENT → DIAGNOSIS → REPAIR → PATCH → PR → REVIEW → CI → VERIFY → MERGE → DEPLOY → MONITOR
```

### Matriz de Transiciones Regulada

| Transición | Actor | Autoridad | Input | Output | Evidencia | Permiso Requerido | Rollback / Fallback |
|---|---|---|---|---|---|---|---|
| **ISSUE → INCIDENT** | Webhook / Monitoring | Automated System | Payload HTTP | `Incident` struct | Webhook Secret Validated | `WEBHOOK_SECRET` | Reject HTTP request (401/503) |
| **INCIDENT → DIAGNOSIS** | Edge Worker | Automated Worker | `Incident` | `FailureSignature` + `FeatureVector[64]` | Computed hash & features | Read-only memory isolate | Drop incident / log failure |
| **DIAGNOSIS → REPAIR** | Edge Worker (NN) | `RepairNet` (2863 weights) | `FeatureVector` | `RepairAction` (Op, Conf, Risk) | Model execution logs | Internal WASM isolate | Fallback to zeros model |
| **REPAIR → PATCH** | Deterministic Operators | `repair_operators` Crate | `RepairAction` | `CandidatePatch` / Advisory | Allowlist validation | `contents: read` | Escalate to human (No-Op) |
| **PATCH → PR** | GitHub App / Bot | DevOps Agent | `CandidatePatch` | GitHub Pull Request | Signed Git Commit & PR ID | Branch write / PR create | Close PR / Delete branch |
| **PR → REVIEW** | Maintainer / Security | Human Owner | PR Diff | Approval / Request Changes | PR Review Comment | Write / Admin repo permissions | Block Merge / Request Changes |
| **REVIEW → CI** | GitHub Actions | Runner / Toolchain | PR Branch Commit | CI Test & Clippy Matrix | `ci.yml` / `wasm.yml` Log Artifacts | Workflow execution | Fail CI / Block PR merge |
| **CI → VERIFY** | Policy Gate Worker | `GOVERNANCE.md` Rules | CI Status + Conf/Risk Scores | Policy Pass/Fail Verdict | Signed `PipelineReport` | Verification gate check | Block PR promotion |
| **VERIFY → MERGE** | Repository Owner | Human Maintainer | VERIFY Verdict + CI PASS | Git Merge Commit to `main` | Git Commit ID on `main` | Repository Admin / Codeowner | Git Revert commit |
| **MERGE → DEPLOY** | GitHub Actions / Wrangler | Production Environment | `main` Commit | Edge Worker Deployment | Wrangler Deploy Logs & Smoke Test | `production` env + reviewer | Rollback Worker deployment |
| **DEPLOY → MONITOR** | Monitoring / Telemetry | Observability Engine | `/health` & Edge Metrics | Telemetry & Logs | Health Endpoint HTTP 200 | Read-only metrics | Auto-disable webhook / revert |

## Lógica de Gate (Fail-Closed)

```rust
if confidence < MIN_CONFIDENCE -> BLOCK (LowConfidence)
if risk > MAX_RISK             -> BLOCK (HighRisk)
if operator_id unknown         -> BLOCK
if webhook_secret invalid      -> REJECT (401/503 constant-time)
else                           -> ALLOW
```

La red neuronal **sólo propone**. Governance + CI/VERIFY **deciden siempre**.
