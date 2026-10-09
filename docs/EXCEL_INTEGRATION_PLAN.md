# Plan de integracion — repos Rust/Cloudflare evaluados (2026-10-09)

Fuente: inventario externo de 49 repos Rust/Cloudflare (oficiales Cloudflare,
comunidad y repair/self-healing). Evaluacion contra los invariantes del repo:
Rust/WASM, fail-closed, $0/mes (Always Free), Actions = unica autoridad,
sin generacion libre de codigo, el LLM jamas ve el arbol del repo.

## Integrado (PART9-PART10)

- **Paridad de diffs** (PART9): `crates/repair_pr/tests/diff_parity.rs`
  compara el generador runtime (`repair_operators::unified_diff`) contra el
  de evidencia offline (`repair_pr` / similar) dentro de
  `cargo test --workspace`. El riesgo de divergencia del DEAD_CODE_AUDIT
  queda cerrado en CI.
- **agentjson (concepto, no dependencia)** (PART10):
  `worker/src/worker/llm_fallback.rs` gana `repair_candidate_json`:
  strict-parse primero y, solo si falla, un pase determinista que strippa
  fences de markdown y comas colgantes (respetando strings/escapes).
  agentjson distribuye bindings Python (PyO3): NO es importable en el WASM
  del worker, asi que se adopta su pipeline, no su crate. El gate
  (operadores permitidos + clamp01 + fail-closed) no cambia: cada llamada
  LLM rescatada es presupuesto del free tier que antes se perdia.

## Fase 3 — cloudflare-rs (operadores infra-repair) [diseño, no implementado]

Objetivo: operadores de remediacion de INFRA (purgar cache, corregir
AGENT_CONFIG, DNS) con el mismo gate. Requisitos antes de codear:

1. `cloudflare-rs` compila a wasm32-unknown-unknown con `worker::Fetch`
   (verificar: el worker ya evito octocrab en runtime por esto).
2. `OperatorId` es contrato de la NN (13 salidas): NUEVOS operadores
   exigen re-entrenar V1 (repair_train) y actualizar feature schema si
   toca. No es un cambio aditivo trivial.
3. Governance: infra-repair es write sobre produccion =>
   `PRODUCTION_WRITE=false` lo bloquea hoy. Requiere decision del dueno
   (nueva categoria advisory-only primero).

## Fase 4 — wirefilter + saffron [diseño, no implementado]

- **wirefilter**: `REPAIR_RULES` pasa de JSON ad-hoc (`rules.rs`) a
  expresiones estilo Wireshark auditables. wirefilter es crate normal
  (no_std friendly?) — verificar antes; el formato actual ya es fail-closed.
- **saffron**: schedules de MONITOR mas ricos que un solo cron horario.
  El cron trigger vive en wrangler.toml (plataforma), saffron evaluaria
  POLITICAS dentro del handler scheduled.

## Descartados y por que

- quiche/boringtun/moq-rs/odoh-rs/boring/lol-html/networkquality-rs/
  mmap-sync/rustwasm-worker-template: problemas de red/transporte que un
  agente de reparacion de CI no tiene.
- Aether/sni-spoofing-rust: evasion DPI/anti-censura, fuera de alcance.
- OptiVorbis: reparacion de audio, sin relacion.
- zeroshot/opencrabs/amux/SoloDawn (como dependencias): LLM-first,
  contradicen la politica de LLM acotado. Referencia conceptual unicamente.
- river/pingap/zentinel/aralez/pingora-proxy-manager/pingoo: solo si el
  agente ampliara a "curar proxies" (futuro, no hoy).
- freighter/foundations/shellflip/ecdysis/ammonia/svg-hush/edgesearch/
  takumi/cloudflare-worker-image/workers-tunnel/cloudflare-ddns/
  cloudflare-speed-cli/open-compute/rivet: util solo si crece el alcance
  (self-host, binario long-running, HTML no confiable, reportes visuales).
