# PART 1 – Repositorio y Estructura GitHub

> **Repositorio canónico**: `Rigohl/auto-healing-agent` (GitHub) – source of truth único.
> No se crea repositorio separado. Todo bajo `auto-healing-agent`.

## Estructura objetivo

```
auto-healing-agent/
├── .github/workflows/   # ci, wasm, consistency, security, regression, repair-validation, auto-repair, deploy, deploy-staging, promote-model, cleanup-branches
├── crates/
│   ├── repair_types/    # Incident, RepairAction, RepairCase, OperatorId
│   ├── feature_engine/  # Incident → [f32; 64]
│   ├── repair_nn_core/  # MLP no_std (64→32→16 + heads)
│   ├── repair_nn_wasm/  # wasm-bindgen adapter
│   ├── repair_operators/# deterministic AST operators + gate
│   └── repair_train/    # trainer offline V1 (SGD determinista, bin repair-train)
├── worker/              # Cloudflare workers-rs (orchestrator only)
├── model/               # schema.json, current.json, stable.json, current.txt (payload KV real)
├── docs/                # ARCHITECTURE, GOVERNANCE, PART*, PROMPT_PAD…
├── legacy/              # archived TS/JS (not executed)
├── scripts/
├── Cargo.toml
├── rust-toolchain.toml
└── README.md
```

## Principios
1. Un solo workspace Rust.
2. Crates desacoplados (responsabilidad única).
3. Worker orquesta; no calcula.
4. Modelos versionados en `model/` (`current` activo, `stable` rollback).
5. Tests inline en crates (no directorio `tests/` vacío).
6. Scripts de train/export fuera de Cloudflare.

## Responsabilidad por crate

| Crate | Responsabilidad | no_std |
|-------|-----------------|--------|
| repair_types | Tipos compartidos | Sí |
| feature_engine | Incident → [f32; 64] | Sí |
| repair_nn_core | Forward pass NN | Sí |
| repair_nn_wasm | Adaptador wasm-bindgen | No |
| repair_operators | Operadores deterministas + gate | Parcial |
| repair_train | Entrenamiento offline V1 + export del payload | No (std, fuera del edge) |
| worker | Orquestador CF | No |

## Fases Prompt Pad relacionadas
FASE 1 (inventario), FASE 2 (types + features), FASE 3 (NN + operators), FASE 6 (worker).

## Regla transversal
Antes de crear crates o configs, verificar versiones actuales vía Context7 / GitHub MCP / docs.rs (cargo, workers-rs, wasm-bindgen, wasm-opt).
