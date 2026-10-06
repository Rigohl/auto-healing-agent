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
| Deterministic operator → CandidatePatch + diff | ✅ (2026-10-05) `apply()` genera el `CandidatePatch` y `crates/repair_operators/src/diff.rs` (`generate_edit` + `unified_diff`) produce el diff unificado acotado: edits `from`→`to` o bumps de versión dentro del mismo major; advisory ops y `LockfileRefresh` nunca emiten diff; archivos > 256 KiB rechazados. AST real = fase posterior |
| Worker: webhook → Incident → features → NN → gate | ✅ `worker/src/lib.rs` valida el secret, calcula `correlation_id`/`idem_key`, consulta al DO (`/ingest`) y encola; el consumidor corre features → NN → gate. Devuelve `status`/`correlation_id`/`preview` (no un `PipelineReport`). |
| Worker: verificación de secret | ✅ **fail closed**: sin `WEBHOOK_SECRET` responde 503, no acepta tráfico |
| Pesos reales | ⚠️ `model/current.txt` = payload KV real (2863 `f32`) entrenado por `crates/repair_train` (V1, sin LLM) y validado en CI contra el `extract`/`predict` reales. Sin `model/current`/`model/stable` **en KV** el Worker sigue respondiendo `blocked_no_model` (fail-closed, red de ceros prohibida): la promoción es manual. |
| PR efímera + Actions VERIFY | ✅ (2026-10-05) `worker/src/worker/github_client.rs` (rama `auto-heal/{cid}` → commit Contents API → PR vía `worker::Fetch`, sin octocrab) llamado por `attempt_repair()` en `queue_consumer.rs`; VERIFY corre en Actions y reporta a `POST /github/callback` (fail-closed con `WEBHOOK_SECRET`). Operativo tras deploy + `GITHUB_TOKEN` (requisito humano). |
|
 RepairCase + TrainingExample on PASS | ❌ sin persistencia |
| Offline train / export / promote R2-KV | ⚠️ train + export implementados (`crates/repair_train`, λ fijada en 0.5, payload `model/current.txt`, bin `repair-train`); falta la promoción a KV/R2 (acción humana; R2 no habilitado en el plan Free). |
| Edge carga pesos STABLE | ⚠️ lee `MODEL_KV:model/current` y, si falla, `model/stable`; `wrangler.toml` ya enlaza el namespace real (`73014a1b32b7446397461a8d438c8ab2`), pero las claves aún no tienen pesos. |

**Full E2E: código completo en `main` (pasos 1–4 del roadmap, 2026-10-05); pendiente el deploy real del Worker + secrets `GITHUB_TOKEN`/`WEBHOOK_SECRET` (acciones humanas, ver Linear PYH-61/PYH-62) para marcarlo verde en producción.** No se declara PASS por confidence del modelo:
la autoridad es GitHub Actions (`docs/NO_LLM_POLICY.md`).