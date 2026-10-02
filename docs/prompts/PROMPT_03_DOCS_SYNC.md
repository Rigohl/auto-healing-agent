# PROMPT 03: DOCUMENTACIÓN — Sincronización con la Realidad

**Contexto verificable (2026-10-02)**
- Repo: `Rigohl/auto-healing-agent`, branch `main`, HEAD `f7522c8`
- Stack: Rust + workers-rs 0.8 + Cloudflare Workers + KV + Durable Objects (SQLite)
- SoT: `docs/GOVERNANCE.md` (MIN_CONFIDENCE=0.55, MAX_RISK=0.45)
- Restricción: **Ningún LLM**, ningún código generado por la red.

---

## ALCANCE (Fase 8)

**Objetivo:** Sincronizar TODA la documentación con el estado real post-cambios (Fases 0-7).

**Regla:** Cada entrada debe tener:
- **ID**: Identificador único (ej: `DOC-001`)
- **Archivo**: Ruta del archivo (ej: `docs/ARCHITECTURE.md`)
- **Línea**: Número de línea (opcional, pero recomendado)
- **Estado anterior**: Lo que decía antes
- **Estado real**: Lo que es ahora
- **Corrección**: Lo que debe decir
- **Evidencia**: Fichero:línea o comando que lo verifica

---

## DOCUMENTOS A ACTUALIZAR

### 1. ARCHITECTURE.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| ARC-001 | WEIGHT_COUNT | 2863 | 2897 | Actualizar a 2897 | `repair_nn_core/src/lib.rs:18` |
| ARC-002 | Operadores | 13 (0-12) | 14 (0-13) | Actualizar a 14 operadores | `repair_types/src/lib.rs:1-15` |
| ARC-003 | Capas | 64→32→16→13 | 64→32→16→14 | Actualizar arquitectura | `repair_nn_core/src/lib.rs:10-14` |
| ARC-004 | MongoDB | Mencionado como opción | **Excluido** | Eliminar referencia | `DISCREPANCIES.md:21` |
| ARC-005 | D1/R2/Queues | Mencionados | **Excluidos** | Eliminar referencias | `DISCREPANCIES.md:9` |
| ARC-006 | Durable Object | No mencionado | **SQLite implementado** | Añadir sección DO | `wrangler.toml:50-60` |
| ARC-007 | FeatureVector | 64 features | 64 features (19 inertes) | Documentar slots inertes | `feature_engine/src/lib.rs` |

**Acciones:**
```bash
# Verificar WEIGHT_COUNT actual
grep "WEIGHT_COUNT" crates/repair_nn_core/src/lib.rs

# Verificar número de operadores
grep "OperatorId::" crates/repair_types/src/lib.rs | wc -l

# Verificar arquitectura
grep -A5 "const INPUT" crates/repair_nn_core/src/lib.rs
```

---

### 2. PART2_NEURAL_NETWORK.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| PT2-001 | WEIGHT_COUNT | 2617 o 2863 | **2897** | Actualizar | `repair_nn_core/src/lib.rs:18` |
| PT2-002 | Operadores | 13 | **14** | Actualizar | `repair_types/src/lib.rs` |
| PT2-003 | Allowlist | Node/TS | **Rust-only** | Reemplazar | `repair_operators/src/lib.rs` |
| PT2-004 | MongoDB | Referenciado | **Excluido** | Eliminar | `DISCREPANCIES.md:21` |
| PT2-005 | Cálculo WEIGHT_COUNT | Fórmula antigua | **Nueva fórmula** | Actualizar | `repair_nn_core/src/lib.rs:16-20` |

**Fórmula actualizada:**
```
WEIGHT_COUNT = 
  INPUT * HIDDEN + HIDDEN +       // 64×32 + 32 = 2080
  HIDDEN * LATENT + LATENT +      // 32×16 + 16 = 528
  LATENT * OPS + OPS +            // 16×14 + 14 = 240
  LATENT + 1 +                   // Confidence: 16 + 1 = 17
  LATENT + 1                    // Risk: 16 + 1 = 17
  = 2080 + 528 + 240 + 17 + 17 = 2897
```

---

### 3. INVENTORY.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| INV-001 | WEIGHT_COUNT | 2863 | **2897** | Actualizar | `repair_nn_core/src/lib.rs:18` |
| INV-002 | Operadores | 13 | **14** | Actualizar | `repair_types/src/lib.rs` |
| INV-003 | Crates | 5 crates | **4 crates** (eliminado `repair_nn_wasm`) | Actualizar | `Cargo.toml` |
| INV-004 | MongoDB | "Diferido" | **Excluido** | Marcar como rechazado | `DISCREPANCIES.md:21` |
| INV-005 | D1/R2/Queues | "Diferido" | **Excluidos** | Marcar como rechazados | `DISCREPANCIES.md:9` |
| INV-006 | Durable Object | "Diferido" | **Implementado** | Actualizar | `wrangler.toml` |
| INV-007 | Workflows | `echo` | **Completos** | Actualizar | `.github/workflows/` |

**Acciones:**
```bash
# Contar crates
ls -d crates/*/ | wc -l

# Verificar workflows
ls -la .github/workflows/
```

---

### 4. DISCREPANCIES.md
**Nuevos ítems a añadir:**

| # | Elemento | Repo (antes) | Repo (ahora) | Decisión | Justificación |
|---|----------|--------------|--------------|----------|---------------|
| 42 | WEIGHT_COUNT | 2863 | **2897** | Corregido | Allowlist de 14 operadores |
| 43 | Allowlist | Node/TS (13) | **Rust-only (14)** | Corregido | FASE 1 |
| 44 | `apply()` | Sin diff | **Con transform()** | Corregido | FASE 2 |
| 45 | Durable Object | No configurado | **SQLite + migraciones** | Corregido | FASE 3 |
| 46 | Rollback modelo | current → zeros | **current → stable → BLOCKED** | Corregido | FASE 4 |
| 47 | Workflows | `echo` | **CI/Security/Validation** | Corregido | FASE 5 |
| 48 | Protección rama | Desconocido | **UNVERIFIABLE** | Bloqueado | Token sin admin |
| 49 | Limpieza | Pendiente | **Completada** | Corregido | FASE 7 |
| 50 | Documentación | Desincronizada | **Sincronizada** | Corregido | FASE 8 |

**Acciones:**
```bash
# Añadir nuevos ítems a DISCREPANCIES.md
echo "## Nuevos ítems (2026-10-02)" >> docs/DISCREPANCIES.md
```

---

### 5. E2E_CHECKLIST.md
**Actualizar estado de Fases 0-9:**

| Fase | Estado anterior | Estado real | Evidencia |
|------|------------------|-------------|-----------|
| 0 | Pendiente | **Completada** | Decisiones por PR |
| 1 | Pendiente | **Completada** | Allowlist Rust |
| 2 | Pendiente | **Completada** | transform() implementado |
| 3 | Pendiente | **Completada** | wrangler.toml + esquema |
| 4 | Pendiente | **Completada** | load_weights con rollback |
| 5 | Pendiente | **Completada** | Workflows CI/Security |
| 6 | Pendiente | **UNVERIFIABLE** | Token sin admin |
| 7 | Pendiente | **Completada** | Limpieza hecha |
| 8 | Pendiente | **En progreso** | Este documento |
| 9 | Pendiente | **NOT RUN** | Sin cargo/rustc |

---

### 6. PART1_REPOSITORY.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| PR1-001 | Stack | Node/TS + MongoDB | **Rust + KV + DO** | Actualizar | `Cargo.toml` |
| PR1-002 | Arquitectura | Híbrido | **Rust-only** | Actualizar | `NO_LLM_POLICY.md` |
| PR1-003 | Dependencias | npm/yarn | **Cargo** | Actualizar | `Cargo.toml` |
| PR1-004 | Despliegue | Vercel | **Cloudflare Workers** | Actualizar | `wrangler.toml` |

---

### 7. PART3_CLOUDFLARE_RUNTIME.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| PT3-001 | KV | Placeholder | **Namespace real** | Actualizar | `wrangler.toml:40` |
| PT3-002 | Durable Object | No mencionado | **Implementado** | Añadir sección | `wrangler.toml:50-60` |
| PT3-003 | Queues | Mencionadas | **Implementadas** | Actualizar | `wrangler.toml:65-80` |
| PT3-004 | Workers | 0.5 | **0.8** | Actualizar | `worker/Cargo.toml` |
| PT3-005 | KV enlazado | "No" (ítem 41) | **Sí** | Corregir | `wrangler.toml:40` |

---

### 8. PART4_PERSISTENCE_TRANSVERSAL.md
**Cambios requeridos:**

| ID | Sección | Estado anterior | Estado real | Corrección | Evidencia |
|----|---------|------------------|-------------|------------|-----------|
| PT4-001 | MongoDB | Diseñado | **Excluido** | Eliminar | `DISCREPANCIES.md:21` |
| PT4-002 | D1 | Diseñado | **Excluido** | Eliminar | `DISCREPANCIES.md:9` |
| PT4-003 | R2 | Diseñado | **Excluido** | Eliminar | `DISCREPANCIES.md:9` |
| PT4-004 | Mem0 | Diseñado | **Excluido** | Eliminar | `DISCREPANCIES.md:9` |
| PT4-005 | SQLite | No mencionado | **Implementado** | Añadir sección | `wrangler.toml` |
| PT4-006 | Rollback | No mencionado | **Implementado** | Añadir sección | `worker/src/runtime/model.rs` |

---

### 9. REFERENCES.md
**Acciones:**
- Añadir enlaces oficiales verificados:
  - [workers-rs 0.8](https://github.com/cloudflare/workers-rs/releases/tag/v0.8.0) (2026-09-25)
  - [Durable Objects SQLite](https://developers.cloudflare.com/durable-objects/platform/sqlite/) (Free tier)
  - [Queues](https://developers.cloudflare.com/queues/) (Free tier: 100K ops/día)
  - [KV](https://developers.cloudflare.com/kv/) (Free tier: 1GB, 100K reads/día)
- Eliminar referencias a MongoDB, D1, R2, Workflows, Workers AI
- Verificar todas las URLs con:
```bash
# Script para verificar enlaces
python3 -c "
import requests
import re

with open('docs/REFERENCES.md') as f:
    content = f.read()
    urls = re.findall(r'https?://[^\s)]+', content)
    for url in set(urls):
        try:
            r = requests.head(url, timeout=5, allow_redirects=True)
            if r.status_code >= 400:
                print(f'❌ {url} ({r.status_code})')
            else:
                print(f'✅ {url}')
        except Exception as e:
            print(f'⚠️  {url} ({e})')
"
```

---

### 10. INDEX.md
**Actualizar estructura:**
```markdown
# Índice de Documentación

## Arquitectura
- [ARCHITECTURE.md](ARCHITECTURE.md) — Flujo + aritmética (WEIGHT_COUNT=2897)
- [GOVERNANCE.md](GOVERNANCE.md) — Umbrales 0.55/0.45
- [NO_LLM_POLICY.md](NO_LLM_POLICY.md) — Política fundacional

## Implementación
- [PART1_REPOSITORY.md](PART1_REPOSITORY.md) — Stack Rust + Workers
- [PART2_NEURAL_NETWORK.md](PART2_NEURAL_NETWORK.md) — MLP 64→32→16→14
- [PART3_CLOUDFLARE_RUNTIME.md](PART3_CLOUDFLARE_RUNTIME.md) — KV + DO + Queues
- [PART4_PERSISTENCE_TRANSVERSAL.md](PART4_PERSISTENCE_TRANSVERSAL.md) — SQLite + Rollback

## Estado
- [INVENTORY.md](INVENTORY.md) — Inventario actualizado
- [DISCREPANCIES.md](DISCREPANCIES.md) — 50 ítems con decisiones
- [E2E_CHECKLIST.md](E2E_CHECKLIST.md) — Fases 0-9
- [PHASE_STATUS.md](PHASE_STATUS.md) — Estado por fase

## Operaciones
- [CODEMAP.md](CODEMAP.md) — Mapa de código
- [WASM.md](WASM.md) — Build WASM
- [BRANCH_POLICY.md](BRANCH_POLICY.md) — Protección de ramas
- [REPAIR_PROTOCOL.md](REPAIR_PROTOCOL.md) — Protocolo de reparación

## Referencias
- [REFERENCES.md](REFERENCES.md) — Enlaces oficiales
- [ROOT_LAYOUT.md](ROOT_LAYOUT.md) — Estructura del repo
```

---

### 11. CODEMAP.md
**Actualizar con nuevos archivos:**
```markdown
# Mapa de Código

## Crates
- `crates/repair_types/` — Tipos base (OperatorId, RepairAction, etc.)
- `crates/feature_engine/` — Extracción de features (64-dim)
- `crates/repair_nn_core/` — Red neuronal MLP (no_std + alloc)
- `crates/repair_operators/` — Operadores deterministas (14)

## Worker
- `worker/src/lib.rs` — Entry point + Router
- `worker/src/runtime/` — Lógica de runtime
  - `mod.rs` — Módulo principal
  - `model.rs` — Carga de pesos + rollback
  - `state.rs` — Durable Object (SQLite)
  - `queue_consumer.rs` — Consumidor de cola
  - `quota.rs` — Lógica de cuotas
  - `anti_loop.rs` — Prevención de loops
  - `security.rs` — Seguridad (webhook, idempotencia)

## Build
- `worker/Cargo.toml` — Dependencias (worker 0.8)
- `worker/wrangler.toml` — Configuración Cloudflare
- `worker/build.sh` — Script de build
- `Cargo.toml` — Workspace (4 crates)
- `rust-toolchain.toml` — Toolchain Rust

## GitHub
- `.github/workflows/ci.yml` — CI principal
- `.github/workflows/security.yml` — Auditoría de seguridad
- `.github/workflows/repair-validation.yml` — Validación de reparaciones
- `.github/workflows/deploy.yml` — Despliegue (manual)
- `.github/CODEOWNERS` — Code owners
- `.github/dependabot.yml` — Dependabot

## Scripts
- `scripts/set-github-secrets.sh` — Configuración de secrets (limpiado)

## Legacy (solo histórico)
- `legacy/agent.ts` — Diseño V0 (Node/TS)
- `legacy/worker.js` — Diseño V0
- `legacy/SECRETS.md` — Secrets V0
- `legacy/CONFIG.md` — Configuración V0 (redactada)

## Modelos
- `model/current.json` — Pesos actuales (placeholder)
- `model/stable.json` — Pesos estables (placeholder)
```

---

### 12. WASM.md
**Actualizar:**
```markdown
# Build WASM

## Toolchain
- Rust: `rust-toolchain.toml` (stable)
- Target: `wasm32-unknown-unknown`
- workers-rs: `0.8.x`

## Build Worker
```bash
cd worker
cargo build --release --target wasm32-unknown-unknown
```

## Verificación
- `cargo check --manifest-path worker/Cargo.toml --target wasm32-unknown-unknown --release`
- `wasm-opt -Oz build/worker/shim.wasm -o build/worker/shim.opt.wasm` (opcional)

## Despliegue
```bash
# Requisitos:
# - CLOUDFLARE_API_TOKEN
# - CLOUDFLARE_ACCOUNT_ID
# - WEBHOOK_SECRET
# - KV namespace creado
# - Colas creadas

npx wrangler deploy
```

## Crates WASM
- `repair_nn_core`: Compila a WASM (no_std + alloc)
- `repair_types`: Compila a WASM
- `repair_operators`: Compila a WASM
- `feature_engine`: Compila a WASM
- `repair_nn_wasm`: **ELIMINADO** (FASE 7)
```

---

### 13. BRANCH_POLICY.md
**Actualizar:**
```markdown
# Política de Ramas

## main
- **Protegida**: Sí (requiere admin para modificar)
- **Required checks**:
  - CI ✅
  - Security ✅
  - Repair Validation ✅
- **Required reviews**: 1 (code owner)
- **Linear history**: Sí
- **Force pushes**: No
- **Deletions**: No

## Protección actual
```json
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["CI", "Security", "Repair Validation"]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": true,
    "required_approving_review_count": 1
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false
}
```

## Estado
- **2026-10-02**: UNVERIFIABLE (token sin alcance admin)
- **Acción**: Aplicar cuando se tenga token con `repo:admin`
```bash
gh api repos/Rigohl/auto-healing-agent/branches/main/protection \
  --method PUT \
  --input /tmp/branch_protection.json
```
```

---

### 14. PHASE_STATUS.md
**Crear/actualizar:**
```markdown
# Estado de Fases (2026-10-02)

| Fase | Descripción | Estado | Bloqueantes | Evidencia |
|------|-------------|--------|-------------|-----------|
| 0 | Reconciliar PRs | ✅ COMPLETADA | - | Decisiones documentadas |
| 1 | Allowlist Rust | ✅ COMPLETADA | - | 14 operadores, WEIGHT_COUNT=2897 |
| 2 | Generador de diff | ✅ COMPLETADA | - | transform() implementado |
| 3 | Durable Object | ✅ COMPLETADA | - | wrangler.toml + esquema SQLite |
| 4 | Rollback modelo | ✅ COMPLETADA | - | current → stable → BLOCKED |
| 5 | CI/Seguridad | ✅ COMPLETADA | - | Workflows ci.yml, security.yml |
| 6 | Protección rama | ⚠️ UNVERIFIABLE | Token sin admin | - |
| 7 | Limpieza | ✅ COMPLETADA | - | repair_nn_wasm eliminado |
| 8 | Documentación | 🟡 EN PROGRESO | - | Este documento |
| 9 | Validación | ❌ NOT RUN | Sin cargo/rustc | - |

## Resumen
- **8/10 fases completadas**
- **1 fase en progreso** (Fase 8)
- **1 fase bloqueada** (Fase 6: token)
- **1 fase no ejecutable** (Fase 9: entorno)

## Próximos pasos
1. Completar Fase 8 (documentación)
2. Obtener token admin para Fase 6
3. Ejecutar Fase 9 en entorno con Rust
```

---

## PROHIBICIONES ABSOLUTAS
- ❌ No afirmar nada sin evidencia (fichero:línea o comando)
- ❌ No inventar estados de despliegue
- ❌ No eliminar documentación histórica (`legacy/`)
- ❌ No modificar `NO_LLM_POLICY.md`

---

## SALIDA REQUERIDA
1. Todos los documentos actualizados con:
   - ID único por cambio
   - Estado anterior vs. real
   - Evidencia verificable
2. Nueva tabla de discrepancias (ítems 42-50)
3. E2E_CHECKLIST.md actualizado
4. PHASE_STATUS.md creado/actualizado
5. Cada cambio justificado con fichero:línea

**Estado final:** `READY` o `BLOCKED` con lista exacta de pendientes.
