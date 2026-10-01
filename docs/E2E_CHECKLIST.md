# FASE 9 — E2E checklist

> Absorbida de `feat/rust-nn-core:docs/E2E_CHECKLIST.md` (commit `a886669`).
> Traza el flujo `Incident → NN → gate → PR → VERIFY` de `docs/ARCHITECTURE.md`.

- [ ] Incident normalized from webhook
- [ ] Evidence collected
- [ ] FailureSignature stored
- [ ] Features[64] computed
- [ ] NN WASM → RepairAction
- [ ] Governance gate
- [ ] Deterministic operator → CandidatePatch
- [ ] Ephemeral PR ref
- [ ] Actions compile/test/regression
- [ ] Verify PASS/FAIL (not confidence)
- [ ] RepairCase + TrainingExample on PASS
- [ ] Offline train / checkpoint / R2-KV promote
- [ ] Edge loads STABLE weights only

## Estado

Marcado lo que existe hoy en `main`:

| Paso | Estado |
|------|--------|
| Types + `FailureSignature` + `FeatureVector[64]` | ✅ `crates/repair_types`, `crates/feature_engine` |
| NN WASM → RepairAction | ✅ `crates/repair_nn_core`, `crates/repair_nn_wasm` (build en CI) |
| Governance gate | ✅ `repair_operators::gate` (0.55 / 0.45) |
| Deterministic operator → CandidatePatch | ⚠️ stubs; AST real = FASE posterior |
| Worker (webhook / health / model ptr) | ⚠️ esqueleto; NN wiring pendiente |
| Webhook → Incident (extremo a extremo) | ❌ no cableado |
| PR efímera + Actions VERIFY | ❌ no cableado |
| RepairCase + TrainingExample on PASS | ❌ sin persistencia |
| Offline train / export / promote R2-KV | ❌ diferido (peso real pendiente) |
| Edge carga pesos STABLE | ❌ `model/` son placeholders (`weights: null`) |

**Full E2E no está verde.** No se declara PASS por confidence del modelo:
la autoridad es GitHub Actions (`docs/NO_LLM_POLICY.md`).