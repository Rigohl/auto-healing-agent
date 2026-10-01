# PART 3 – Cloudflare y Flujo de Runtime

> Worker = orquestador, **no** motor de cómputo. Coste objetivo: $0 (Free tier).

## Flujo completo

```
GitHub webhook
  → Cloudflare Worker (valida secret, normaliza Incident)
  → Feature extraction (feature_engine)
  → Rust/WASM Neural Network → RepairAction
  → Confidence/Risk Gate (0.55 / 0.45)
  → Deterministic Operator → CandidatePatch
  → GitHub PR
  → Actions CI/VERIFY (PASS | FAIL | BLOCKED)
  → RepairCase → MongoDB (+ Notion / Mem0 cuando existan)
```

## Rol del Worker

**Sí**: webhook HMAC, normalización, features, inferencia NN, gate.

**No**: entrenamiento, builds largos, tests de integración, dataset, git completo, generación libre de código.

## Ruta WASM elegida
`Rust → wasm32-unknown-unknown → módulo autocontenido → Worker`
(No Emscripten experimental.)

La NN se **enlaza como crate Rust** (`repair_nn_core`, `no_std` + alloc) dentro del
propio módulo WASM del Worker. Por eso `wrangler.toml` **no** usa
`[wasm_modules]`: `repair_nn_wasm` existe para consumidores JS/navegador, no
para el Worker.

## Límites Free relevantes
CPU 10 ms/req · Mem 128 MB · Bundle 64 MiB · 100k req/día · KV 100k reads / 1k writes · R2 10 GB.

## wrangler.toml (esqueleto)

```toml
name = "auto-healing-agent-worker"
compatibility_date = "2024-09-23"

[[kv_namespaces]]
binding = "MODEL_KV"
id = "<id real del namespace>"
```

> En el repo, `worker/wrangler.toml` conserva `REPLACE_WITH_KV_NAMESPACE_ID`.
> Hay que rellenarlo antes de cualquier `wrangler deploy`; mientras tanto
> `MODEL_KV` queda sin enlazar y `/model` devuelve `kv: unbound`, y el webhook
> corre con red de ceros (`weights: zeros:no_weights`).

## Fases relacionadas
FASE 3 (wasm adapter), FASE 6 (deploy worker), hardening wasm-opt.

## Fuentes
Cloudflare Workers / Rust / WASM docs, workers-rs, limits oficiales.
