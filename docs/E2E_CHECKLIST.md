# FASE 9 — E2E checklist

> Absorbida de `feat/rust-nn-core:docs/E2E_CHECKLIST.md` (commit `a886669`).
> Traza el flujo `Incident → NN → gate → PR → VERIFY` de `docs/ARCHITECTURE.md`.

- [ ] Incident normalized from webhook
- [ ] Evidence collected
- [ ] FailureSignature stored
- [ ] Features[64] computed
- [ ] NN WASM → RepairAction
- [ ] Governance gate
- [x] Deterministic operator → CandidatePatch (diff unificado vía `diff.rs`)
- [x] Ephemeral PR ref (`github_client.rs`: rama `auto-heal/{cid}`)
- [ ] Actions compile/test/regression
- [ ] Verify PASS/FAIL (not confidence)
- [x] RepairCase on PASS (`REPAIR_CASES_KV`); TrainingExample = contrato pendiente
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
| Pesos reales | ⚠️ `model/current.txt` = payload KV real (2863 `f32`) entrenado por `crates/repair_train` (V1, sin LLM) y validado en CI contra el `extract`/`predict` reales. Sin `model/current`/`model/stable` **en KV** el Worker sigue respondiendo `blocked_no_model` (fail-closed, red de ceros prohibida): la promoción es manual. |
| PR efímera + Actions VERIFY | ❌ no implementado; la respuesta incluye `"pr": null` y lo dice |
| RepairCase + TrainingExample on PASS | ✅ (2026-10-05) `persist_case()` guarda el `RepairCase` en `REPAIR_CASES_KV` (clave `repair_case:{correlation_id}`, namespace real verificado 2026-10-06); `TrainingExample` sigue siendo contrato sin consumidor (DISCREPANCIES 74) |
| Offline train / export / promote R2-KV | ⚠️ train + export implementados (`crates/repair_train`, λ fijada en 0.5, payload `model/current.txt`, bin `repair-train`); falta la promoción a KV/R2 (acción humana; R2 no habilitado en el plan Free). |
| Edge carga pesos STABLE | ⚠️ lee `MODEL_KV:model/current` y, si falla, `model/stable`; `wrangler.toml` ya enlaza el namespace real (`73014a1b32b7446397461a8d438c8ab2`), pero las claves aún no tienen pesos. |

**Full E2E no está verde.** No se declara PASS por confidence del modelo:
la autoridad es GitHub Actions (`docs/NO_LLM_POLICY.md`).

## Prevención de errores de Workers Builds (2026-10-06, tras falla real)

Firma de la falla: build sin compilar ("No build output detected to cache") +
deploy en la raiz -> "Could not detect a directory containing static files".
Causa raiz: configuracion del dashboard (Root directory / Build command),
no del repo. Guardas activas y de proceso:

1. **Config correcta (unica fuente)**: Root directory `worker`, Build
   `bash build.sh`, Deploy `npx wrangler deploy`. Alternativa inmune a
   errores de Root directory: Deploy
   `cd worker && bash build.sh && npx wrangler deploy`.
2. **Guarda en build.sh**: si no encuentra wrangler.toml/Cargo.toml a su
   lado, aborta con mensaje explicito (nunca un deploy sin bindings).
3. **Guarda estructural del repo (ACTUALIZADA 2026-10-06)**: la raiz tiene
   ahora un wrangler.toml ESPEJO COMPLETO de worker/wrangler.toml (decision
   del dueno; main=worker/build/..., [build] cwd=worker) para que
   `npx wrangler deploy` en la raiz despliegue con TODOS los bindings SIN
   tocar el dashboard. validate-preflight.sh guard 6 exige equivalencia de
   claves criticas (ids KV, colas, DO, crons, vars): divergencia = CI rojo.
   build.sh sigue PROHIBIDO en la raiz.
4. **Watch paths** (dashboard > Build > watch paths): `worker/**` para que
   solo los cambios del worker disparen builds (menos builds fallidos por
   pushes de docs).
5. **Firmas de diagnostico rapido**:
   - "No build output detected to cache" + deploy sin compilar = falta
     Build command o Root directory mal.
   - "Could not detect a directory containing static files" = wrangler
     sin config en el CWD (corriendo en la raiz).
   - "Workers Builds" fallando en GitHub checks pero CI verde = ambiental
     (dashboard), no del codigo.
6. **Retry sin dashboard**: los builds fallidos se pueden relanzar directo
   desde GitHub (changelog 2025-03-17), y la API de Builds usa el *tag*
   del Worker (UUID), no su nombre.
7. **Verificacion post-deploy**: el worker desplegado debe pesar >100 KB
   (placeholder = 275 bytes), tener los 2 KV bindings + DO + Queues, y
   responder 200 en /health. Ver DEVOPS_STATUS seccion F.
