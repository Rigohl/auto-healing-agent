# Branch Policy & Governance

## Source of Truth
GitHub repository `Rigohl/auto-healing-agent` is the primary Source of Truth.
Direct pushes to `main` are restricted. All changes must arrive via Pull Requests.

## PR Governance & Rulesets
- **Required Checks**:
  - `CI / fmt`
  - `CI / test`
  - `CI / clippy`
  - `CI / wasm`
  - `CI / worker`
  - `Security / Cargo Audit`
  - `Security / Gitleaks Secret Scan`
  - `Repair Validation / validate`
- **Permissions**: Every workflow declares strict `permissions: contents: read` (or specific minimal required scope).
- **Concurrency**: Workflows use `concurrency` cancellation to prevent wasteful pipeline executions.

## Mergify & Automation Integration
If Mergify is enabled on the repository:
- **GitHub Authority**: GitHub rulesets enforce branch protection, required status checks, and required reviews.
- **Mergify Authority**: Mergify handles queue management, dependency updates, and controlled auto-merging after all required checks pass.
- **Agent Health**: Autonomous agents do not have direct merge authority. All automated PRs must clear status checks and policies.
