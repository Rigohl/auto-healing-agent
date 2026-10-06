# Arquitectura de Claude Code — Análisis completo

> Fuente principal: guía "Claude Code Architecture & Agent Loop" (cc.bruniaux.com,
> verificada 2026-10-06) + documentación oficial de Anthropic citada por ella.
> Términos con confianza marcada por la fuente (Tier 1 oficial / Tier 2 verificada /
> Tier 3 inferencia). Análisis hecho a pedido del propietario; aplicabilidad
> filtrada por docs/NO_LLM_POLICY.md de ESTE repo.

## 1. Qué es Claude Code (harness, no modelo)

Claude Code es un **runtime harness**: posee el loop modelo↔herramientas de una
tarea de código. El modelo genera tokens; el repo aporta instrucciones, setup,
estado y verificación; un orquestador coordina sesiones. No es un modelo nuevo:
es una capa de orquestación sobre Claude (Opus/Sonnet/Haiku).

## 2. El master loop (Tier 1, oficial)

Todo el sistema es un `while`:

```
while (respuesta.tiene_tool_call):
    resultado = ejecutar_herramienta(tool_call)
    respuesta = enviar_a_claude(resultado)
return respuesta.texto
```

**NO existe**: clasificador de intenciones, router de tareas, pipeline
RAG/embeddings, orquestador DAG, split planner/executor. El modelo decide todo:
qué herramienta llamar, cuándo y cuándo terminó (`stop_reason`: `tool_use` |
`end_turn` | `max_tokens`).

Razones del diseño: menos componentes = menos modos de fallo; el razonamiento
del modelo supera heurísticas hand-coded; flexibilidad; debuggability.

Límite de profundidad: `max_turns` (5 para lookup simple, 20-30 para
investigación/código multi-paso, 50 para workflows autónomos; al alcanzarlo
el SDK reporta `error_max_turns` — la tarea puede quedar INCOMPLETA).

## 3. Arsenal de herramientas (Tier 1)

| Herramienta | Propósito |
|---|---|
| `Bash` | adaptador universal (el modelo está entrenado en shell) |
| `Read` / `Edit` / `Write` | archivos; `Edit` es diff-based con match exacto |
| `Grep` / `Glob` | búsqueda; ripgrep **reemplazó** a un RAG de embeddings (Voyage): benchmarks internos mostraron mejor rendimiento sin sync de índices ni exposición a proveedores externos ("Search, Don't Index") |
| `Agent` (ex `Task`) | sub-agentes con contexto propio |
| `TodoWrite` | tracking (deprecado por Tasks API) |

Extensión por MCP servers (Serena símbolos, Context7 docs, Playwright, etc.)
y 11 capacidades nativas: hooks de eventos, sub-agentes de fondo, /explore,
/plan, agent tool, agent teams (experimental), selección de modelo por tarea,
MCP, modos de permiso, memoria de sesión (CLAUDE.md).

## 4. Gestión de contexto (Tier 2 con respaldo de investigación)

- Ventana compartida: instrucciones + historial + resultados de tools + respuesta
  (~200K–1M según modelo; usable ≈140-150K tras system prompt y reserva).
- **Auto-compaction**: al superar umbral (~92-95% observado), resume turnos
  viejos y condensa resultados. Investigación citada: la calidad CAE 50-70%
  en tareas complejas al crecer el contexto (Context Rot, Jul 2025); compactar
  pierde matices y referencias → consenso: `/compact` manual en breakpoints
  lógicos (85% = handoff manual recomendado).
- **Degradación predecible**: 15-25 turnos → pierde constraints tempranas;
  >5 archivos simultáneos → inconsistencias. Tasa de éxito: 1-3 archivos ~85%,
  4-7 ~60%, 8+ ~40%.
- **Context drift por fallos**: los errores de herramientas acumulan ruido;
  los reintentos siguen la narrativa del error, no la meta. Patrón documentado:
  re-inyectar la instrucción central tras cada fallo (hook PostToolUse).
- Estrategias: sub-agentes para exploración, /compact manual, /clear, lecturas
  específicas, CLAUDE.md para contexto persistente.

## 5. Sub-agentes (Tier 1)

- Contexto PROPIO y fresco; devuelven SOLO un resumen al padre (protege el
  contexto principal).
- Anidación configurable (3 capas por defecto); tipos: Explore (solo lectura),
  Plan (sin Edit/Write), Bash, general-purpose.
- Patrón dominante en producción: **hub-and-spoke** — un coordinador descompone,
  pasa contexto EXPLÍCITO a cada worker (el contexto nunca se hereda solo;
  es el error más común en multi-agente), agrega resultados y resuelve lo
  transversal. Los workers no se hablan entre sí; si lo necesitan, la
  descomposición está mal.

## 6. Permisos y seguridad (Tier 1 + Tier 2)

4 capas: (1) prompts interactivos, (2) reglas allow/deny en settings.json
(`Bash(rm -rf *)` etc.), (3) hooks Pre/PostToolUse (validar/auditar/override),
(4) sandbox opcional (filesystem aislado + red restringida). Detección de
patrones peligrosos (`rm -rf`, `sudo`, `curl | sh`, `chmod 777`,
`git push --force`): siempre requieren confirmación.

## 7. Edit tool y persistencia

- `Edit` es diff-based: exige match EXACTO del texto original (old_string) —
  el mismo contrato que `crates/repair_operators/src/diff.rs` (edits acotados
  `from`→`to`; si el patrón no existe, fail-closed `pattern_not_found`).
- Sesión persistente: CLAUDE.md + archivos de memoria + `fork_session`
  (ramas de conversación que comparten historial hasta el punto de fork).

## 8. Filosofía: "Less scaffolding, more model"

Confiar en el razonamiento del modelo en vez de construir orquestación compleja
alrededor.

---

## Aplicabilidad a auto-healing-agent (filtro NO_LLM_POLICY.md)

Este repo NO usa LLM en producción, pero la arquitectura de Claude Code
valida o inspira varios elementos YA presentes y sugiere mejoras:

| Elemento de Claude Code | Equivalente aquí | Estado / lección |
|---|---|---|
| Master loop con `stop_reason` + `max_turns` (límite duro) | Cola con `max_retries=3` → DLQ + hard-stop del DO (`max_attempts_per_incident`) | ✅ Ya implementado, mismo principio: NUNCA loop infinito |
| "Search, Don't Index" (grep > RAG) | Sin embeddings; firma determinista (FNV-1a) para dedup | ✅ Alineado por diseño |
| `Edit` exige match exacto | `diff.rs`: `from` debe existir, `FileTooLarge` >256 KiB, `MissingParam` | ✅ Mismo contrato fail-closed |
| Reglas allow/deny + patrones peligrosos | `gate` (0.55/0.45) + `REPAIR_RULES` declarativas fail-closed + NO_LLM_POLICY | ✅ Ya existía; idea nueva: lista explícita de "patrones peligrosos" como reglas por defecto en rules.rs |
| Re-inyectar la meta tras fallos de tool (anti context drift) | ANTI_LOOP_MAX_SAME_FAILING_VERIFICATION en el DO | ⚠️ Parcial: el DO corta bucles de fallos repetidos pero no re-inyecta contexto; en un pipeline sin LLM no hay drift, solo bucle — cubierto |
| Hub-and-spoke: contexto explícito entre agentes | Worker único determinista (sin multi-agente) | N/A por diseño; si algún día hay multi-agente: la regla "contexto nunca se hereda solo" es la #1 |
| Sub-agente devuelve SOLO resumen | `persist_case` guarda un `RepairCase` compacto en KV, no todo el payload | ✅ Mismo patrón de compresión en los bordes |
| Memoria de sesión (CLAUDE.md) | Knowledge/DO/KV separados por responsabilidad | ✅ Ya aplicado (principio 4 de INDEX.md) |
| Fork de sesión para explorar variantes | Ramas `auto-heal/{cid}` efímeras por reparación | ✍️ Posible mejora futura: intentar VARIOS operadores candidatos en ramas separadas y dejar que VERIFY elija (hoy: 1 acción por NN) |

### Conclusión

La arquitectura de Claude Code es "lo mínimo de scaffolding posible alrededor
del modelo". Este proyecto es su espejo determinista: el mismo scaffolding de
seguridad (gates, límites duros, diffs acotados, fail-closed) pero con la NN
como decisor barato y VERIFICABLE en edge, y GitHub Actions como autoridad de
verificación. Las lecciones transferibles ya están en el repo; la única mejora
nueva que inspira es el "multi-candidate branches" (varios operadores → ramas →
VERIFY decide), anotada aquí como idea de backlog.
