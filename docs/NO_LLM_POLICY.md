# No-LLM & Deterministic Action Policy

## Core Principle
Free-form LLM code generation inside the execution pipeline is strictly prohibited.
All repair actions are produced deterministically by struct outputs and typed repair operators.

## Enforced Boundaries
1. **Feature Extraction**: 64-dimensional feature vector (`FeatureVector`).
2. **Neural Inference**: Deterministic lightweight neural network (`RepairNet`).
3. **Policy Gate**: Strict confidence (`>= 0.55`) and risk (`<= 0.35`) threshold gating (`repair_operators::gate`).
4. **Deterministic Patch Generation**: `apply()` operates on allowlisted repair operators.
5. **VERIFY Authority**: Full verification is provided by CI test suites (`cargo test --all`), not neural scores alone.
