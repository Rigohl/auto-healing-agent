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
| NN → RepairAction | ✅ `crates/repair_nn_core`, `crates/repair_nn_wasm` (build en CI) |
| Governance gate | ✅ `repair_operators::gate` (0.55 / 0.45) |
| Deterministic operator → CandidatePatch | ⚠️ `apply()` con un brazo explícito por cada uno de los 13 operadores; genera `CandidatePatch` (files + steps), **no un diff**. AST real = fase posterior |
| Worker: webhook → Incident → features → NN → gate | ✅ `worker/src/lib.rs` valida el secret, calcula `correlation_id`/`idem_key`, consulta al DO (`/ingest`) y encola; el consumidor corre features → NN → gate. Devuelve `status`/`correlation_id`/`preview` (no un `PipelineReport`). |
| Worker: verificación de secret | ✅ **fail closed**: sin `WEBHOOK_SECRET` responde 503, no acepta tráfico |
| Pesos reales | ❌ `model/*.json` con `weights: null`; sin `model/current`/`model/stable` en KV el Worker responde `blocked_no_model` (fail-closed, red de ceros prohibida). |
| PR efímera + Actions VERIFY | ❌ no implementado; la respuesta incluye `"pr": null` y lo dice |
| RepairCase + TrainingExample on PASS | ❌ sin persistencia |
| Offline train / export / promote R2-KV | ❌ diferido (λ sin fijar, ver `PART2_NEURAL_NETWORK.md`) |
| Edge carga pesos STABLE | ⚠️ lee `MODEL_KV:model/current` y, si falla, `model/stable`; `wrangler.toml` ya enlaza el namespace real (`73014a1b32b7446397461a8d438c8ab2`), pero las claves aún no tienen pesos. |

**Full E2E no está verde.** No se declara PASS por confidence del modelo:
la autoridad es GitHub Actions (`docs/NO_LLM_POLICY.md`).