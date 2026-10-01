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

**Sí**: webhook HMAC, normalización, features, inferencia WASM, gate, crear PR, persistir RepairCase.

**No**: entrenamiento, builds largos, tests de integración, dataset, git completo, generación libre de código.

## Ruta WASM elegida
`Rust → wasm32-unknown-unknown → módulo autocontenido → Worker`
(No Emscripten experimental.)

## Límites Free relevantes
CPU 10 ms/req · Mem 128 MB · Bundle 64 MiB · 100k req/día · KV 100k reads / 1k writes · R2 10 GB.

## wrangler.toml (esqueleto)

```toml
name = "auto-healing-agent"
compatibility_date = "2025-01-01"

[wasm_modules]
REPAIR_NN = "./pkg/repair_nn_wasm_bg.wasm"

[[kv_namespaces]]
binding = "MODEL_KV"
id = "<kv-namespace-id>"
```

## Fases relacionadas
FASE 3 (wasm adapter), FASE 6 (deploy worker), hardening wasm-opt.

## Fuentes
Cloudflare Workers / Rust / WASM docs, workers-rs, limits oficiales.
