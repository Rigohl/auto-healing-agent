# GitHub ↔ Cloudflare API Contract & Integration Specification

**Role**: Principal Integration Engineer — API Contracts, GitHub Webhooks, GitHub Actions, Rust Serialization & Distributed Systems
**Status**: Contract Version 1 (`CONTRACT_VERSION = 1`)
**Scope**: Boundary ownership between GitHub and Cloudflare runtime.

---

## 1. Sequence & System Architecture Diagram

```
+------------------+             +--------------------------+             +-------------------------+
|                  |             |                          |             |                         |
|   GitHub Webhook |             | Cloudflare Worker        |             | Durable Object          |
|   (Event Source) |             | (workers-rs / Orchestrator|             | (State / Idempotency)   |
|                  |             |                          |             |                         |
+--------+---------+             +------------+-------------+             +------------+------------+
         |                                    |                                        |
         | 1. POST /webhook (X-GitHub-Delivery)|                                       |
         +----------------------------------->|                                        |
         |                                    | 2. Verify WEBHOOK_SECRET (HMAC)        |
         |                                    | 3. Parse & Validate RepairEvent        |
         |                                    |                                        |
         |                                    | 4. /ingest (repo, incident_id, deliv_id)|
         |                                    +--------------------------------------->|
         |                                    |                                        |
         |                                    | 5. Idempotency & Quota Verdict         |
         |                                    |<---------------------------------------+
         |                                    |    (status: queued / duplicate / limit) |
         |                                    |                                        |
         | 6. HTTP 202 Accepted + correlation_id                                       |
         |<-----------------------------------|                                        |
         |                                    |                                        |
         |                                    | 7. Enqueue task to Queue               |
         |                                    |                                        |
         +------------------------------------+----------------------------------------+
                                              |
                                              v (Async Queue Consumer)
                                              |
                                              | 8. Extract features [64]
                                              | 9. NN Predict -> RepairAction
                                              | 10. Gate Check (MIN_CONF_0.55, MAX_RISK_0.45)
                                              | 11. Operator apply() -> CandidatePatch
                                              |
                                              |--[ If diff missing / no diff generator ]--> BLOCKED
                                              |
                                              v
                                   +----------+----------+
                                   |  Cloudflare → GitHub|
                                   |  PR Creation        |
                                   +----------+----------+
                                              |
                                              | 12. Create Branch & PR (OutboundPRRequest)
                                              v
                                   +----------+----------+
                                   |    GitHub Actions    |
                                   |  (VERIFY Authority) |
                                   +----------+----------+
                                              |
                                              | 13. Runs CI Workflows on PR
                                              | 14. Emit VerifiedResult (PASS/FAIL/BLOCKED/SKIPPED)
                                              v
                                   +----------+----------+
                                   | Cloudflare State DO |
                                   +---------------------+
```

---

## 2. Inbound Interface Contract: GitHub → Cloudflare

### Officially Verified Webhook Events
Per GitHub documentation, auto-repair triggers are restricted to the following verified event types:
1. `issues`: Action `opened` or `labeled` with `auto-repair`.
2. `workflow_run`: Action `completed` with conclusion `failure`.
3. `check_run`: Action `completed` with conclusion `failure`.
4. `pull_request`: Action `opened` or `synchronize`.
5. `repository_dispatch`: Event type `auto_repair_trigger`.

### Inbound Payload Schema (`RepairEvent`)
Rust Type: `crates/repair_types/src/contract.rs::RepairEvent`

| Field | Type | Description |
| :--- | :--- | :--- |
| `contract_version` | `u16` | Active contract version (`1`). Defaulted if omitted. |
| `event` | `GitHubEventType` | Enum: `issues`, `workflow_run`, `check_run`, `pull_request`, `repository_dispatch`. |
| `delivery_id` | `String` | GitHub `X-GitHub-Delivery` header value (UUID). |
| `incident_id` | `String` | Unique incident identifier (e.g., `inc-1710000000`). |
| `repository` | `String` | Full repository name (`owner/repo`). |
| `commit` | `String` | Git commit SHA where failure occurred. |
| `branch` | `String` | Git branch name. Default: `""`. |
| `signature` | `FailureSignature` | Failure signature containing error code, step, family, language, framework, fingerprint. |
| `correlation_id` | `String` | Structured correlation token (`{incident_id}-{fnv_hash}`). |
| `timestamp` | `u64` | Epoch timestamp in milliseconds. |
| `schema_version` | `u32` | Schema version (`1`). Defaulted if omitted. |

---

## 3. Outbound Interface Contract: Cloudflare → GitHub

### Outbound PR Request Schema (`OutboundPRRequest`)
Rust Type: `crates/repair_types/src/contract.rs::OutboundPRRequest`

| Field | Type | Description |
| :--- | :--- | :--- |
| `incident_id` | `String` | Associated incident identifier. |
| `correlation_id` | `String` | Tracking correlation token. |
| `head_branch` | `String` | Created branch (`auto-repair/{incident_id}`). |
| `base_branch` | `String` | Target base branch (`main`). |
| `title` | `String` | PR title (e.g., `[Auto-Repair] Fix dependency lockfile for inc-123`). |
| `body` | `String` | Markdown PR body with failure details and operator metadata. |
| `patch` | `CandidatePatch` | Generated patch details. |
| `labels` | `Vec<String>` | Applied labels (`auto-repair`, `automated-fix`). |
| `metadata` | `BTreeMap<String, String>` | Structured metadata tags. |

### Critical Restriction: CandidatePatch vs Git Diff
The evaluation rule is explicit:
$$\text{apply()} \to \text{CandidatePatch} \neq \text{git diff}$$

`CandidatePatch` contains structured parameters and summaries. A valid GitHub PR requires actual unified diff content.

* **Rule**: NO INVENTED DIFFS.
* **Fallback**: If a diff generator is unavailable (`has_diff_generator == false`) or the diff string is empty, the pipeline status is strictly **`BLOCKED`**, and a P2 dependency is logged against `repair_operators`.

---

## 4. Verification Authority (VERIFY)

GitHub Actions is the **only authority** for verification. Cloudflare orchestrates, enqueues, and creates PRs, but never declares a fix verified or successful on its own.

### Verification Model
$$\text{Actions} \to \{\text{PASS}, \text{FAIL}, \text{BLOCKED}, \text{SKIPPED}\} \to \text{Cloudflare}$$

Rust Type: `crates/repair_types/src/contract.rs::VerifiedResult`

```rust
pub struct VerifiedResult {
    pub result: VerificationResult,
    pub workflow_run_id: String,
    pub commit_sha: String,
    pub evidence_ref: String,
}
```

### Invariant Rules
1. **Evidence Mandate**: No evidence $\to$ `BLOCKED`.
2. **Strict Pass Validation**: It is impossible to construct a valid `VerificationResult::Pass` without non-empty `workflow_run_id`, `commit_sha`, and `evidence_ref` starting with `evidence://` or `https://`.
3. **Never Fallthrough**: Missing CI evidence NEVER defaults to `PASS`.

---

## 5. Idempotency Specification

Any request with identical:
$$\text{idempotency\_key} = \text{FNV-1a-64}(\text{repository} \mid \text{incident\_id} \mid \text{delivery\_id} \mid \text{signature.fingerprint})$$
must yield the same logical operation.

### Key Parameters
* **TTL**: `IDEMPOTENCY_TTL_SECONDS = 86400` (24 hours retention in Durable Object state).
* **Duplicate Response Behavior**:
  * `QueuedOriginal`: First delivery accepted for async processing.
  * `AlreadyProcessed`: Returns existing outcome and PR URL.
  * `InFlight`: Delivery is currently processing in queue.
  * `IgnoredDuplicate`: Secondary duplicate delivery safely ignored with HTTP 200/202.
* **Duplicate PR Behavior**:
  * `return_existing_pr`: Return open PR URL if active PR exists for the incident.
  * `update_branch`: Force update existing auto-repair branch.
  * `skip_duplicate`: Skip PR creation if duplicate branch/PR exists.

---

## 6. Error Matrix & Retry Matrix

Below is the verified matrix mapping HTTP error codes and pipeline error conditions to operational handling:

| Error Condition | Retryable | Send to DLQ | Requires Human | Operational Semantics |
| :--- | :---: | :---: | :---: | :--- |
| **401 Unauthorized** | No | No | Yes | Secret mismatch or invalid authorization token. Fail-closed. |
| **403 Forbidden** | No | No | Yes | Insufficient GitHub App / PAT permissions or rate limit lock out. |
| **404 Not Found** | No | No | Yes | Target repository, branch, or workflow missing. |
| **409 Duplicate** | No | No | No | Duplicate delivery / idempotency match. Safe return of existing state. |
| **429 Rate Limited** | Yes | Eventual | No | GitHub REST API rate limited; exponential backoff then DLQ. |
| **5xx Server Error** | Yes | Eventual | No | Transient upstream server failure; exponential backoff then DLQ. |
| **Invalid Payload** | No | Yes | Yes | Malformed JSON or schema violation; sent directly to DLQ for audit. |
| **VERIFY Missing** | No | No | Yes | Action run or CI evidence missing. Pipeline marks incident `BLOCKED`. |
| **Unsafe Patch** | No | No | Yes | Policy gate rejection (confidence < 0.55 or risk > 0.45). Escalated to human. |

---

## 7. Security Model & GitHub Permissions

### Authentication & Transport Security
* Webhook authorization uses HMAC SHA-256 signatures (`x-hub-signature-256`) or configured secret tokens (`x-webhook-secret`), validated using constant-time comparison (`subtle::constant_time_eq`).
* Unconfigured or missing secrets cause immediate HTTP 503 fail-closed responses.

### Least Privilege GitHub Permissions
Fine-grained PAT or GitHub App must request strictly the minimum required permissions:

```
contents: write          # Create branches and commit repair patches
pull_requests: write     # Create and label repair pull requests
actions: read            # Read workflow execution status for VERIFY
checks: read             # Read check run status for failure detection
metadata: read           # Read repository metadata
```

### Forbidden Token Storage Locations
Tokens must **NEVER** be stored or leaked in:
* Git repositories (source code or commit history)
* `wrangler.toml` files
* Console / worker log outputs
* MongoDB documents or database records
* Notion workspaces or external docs

---

## 8. Versioning Strategy

* **Contract Version Identifier**: `pub const CONTRACT_VERSION: u16 = 1;`
* **Major Version Compatibility**: Any incoming payload with `contract_version > CONTRACT_VERSION` is rejected with `ContractError::UnsupportedMajorVersion`.
* **Forward Evolution**: All struct fields use `#[serde(default)]` where appropriate, allowing backward and forward compatible field additions without breaking existing deserializers. Unknown fields are ignored gracefully.

---

## 9. Crate Dependencies & Execution Status

### Component Dependencies
* **P1 (`repair_types`)**: Core data structures, contract models, idempotency calculation, error policies, evidence validators. **[READY]**
* **P2 (`repair_operators`)**: Deterministic operators and unified diff generation from `CandidatePatch`. **[BLOCKED]** for direct git diff creation until diff generator module is completed.
* **P4 (`worker`)**: Cloudflare Worker runtime, Durable Object state persistence, Queue async dispatch, webhook secret verification. **[READY]**

### Final Status Verdict
* **API Contract & Verification Engine**: **`READY`**
* **End-to-End Git Diff Generation**: **`BLOCKED`** (Dependency on `repair_operators` diff generator).
