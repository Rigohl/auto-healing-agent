//! LLM FALLBACK con presupuesto (Workers AI, plan Always Free).
//!
//! Politica (docs/LLM_POLICY.md, decision del dueno 2026-10-08): la NN sigue
//! siendo la via principal. Cuando el gate la rechaza, se pide al modelo UNA
//! ACCION ESTRUCTURADA (JSON: operador + parametros + confianza + riesgo).
//! El LLM NUNCA escribe codigo libre: su salida pasa por el MISMO gate
//! (0.55/0.45) y los MISMOS operadores deterministas, y GitHub Actions sigue
//! siendo la unica autoridad de VERIFY.
//!
//! Always Free: el plan Free incluye 10.000 Neurons/dia de Workers AI. Este
//! modulo los protege con un contador diario en AGENT_CONFIG (clave
//! llm/budget): sin presupuesto, sin binding o salida invalida => None y el
//! consumidor escala a humano (fail-closed). La request 10.001 nunca falla
//! sola: la puerta se cierra ANTES.
//!
//! Distilacion (el LLM se reemplaza solo): los casos origin=llm que CI
//! verifica PASS quedan en REPAIR_CASES_KV con esa marca; el entrenamiento
//! offline (promote-model) aprende de ellos y la NN pasa a resolver esas
//! firmas sin consultar al LLM (ademas del replay inmediato del ledger).

use serde::{Deserialize, Serialize};
use worker::{console_warn, Env};

use repair_types::{FailureSignature, Incident, OperatorId, RepairAction};

use crate::runtime::config_store::KV_BINDING as CONFIG_KV;

/// Binding de Workers AI (wrangler.toml: [ai] binding = "AI").
pub const AI_BINDING: &str = "AI";
/// Clave del contador diario en AGENT_CONFIG.
pub const KEY_BUDGET: &str = "llm/budget";

pub const VAR_ENABLED: &str = "LLM_ENABLED";
pub const VAR_MODEL: &str = "LLM_MODEL";
pub const VAR_DAILY_BUDGET: &str = "LLM_DAILY_BUDGET";
pub const VAR_COST_PER_CALL: &str = "LLM_COST_PER_CALL";

/// Modelo pequeno del catalogo (barato en Neurons: cabe en el free tier).
/// Verificar contra el catalogo vigente: los modelos grandes (GLM/Kimi/
/// DeepSeek-Pro) ya requieren plan pagado desde 2026-07.
pub const DEFAULT_MODEL: &str = "@cf/meta/llama-3.2-3b-instruct";
/// Tope conservador: deja margen sobre las 10.000 Neurons gratis/dia.
pub const DEFAULT_DAILY_BUDGET: i64 = 8_000;
/// Estimacion conservadora por llamada (Neurons) del modelo pequeno.
pub const DEFAULT_COST_PER_CALL: i64 = 300;

const SYSTEM_PROMPT: &str = "You are a build-repair action classifier. Reply with ONLY a JSON object, no prose, no markdown fences. Schema: {\"operator\": <int>, \"parameters\": {\"dependency\": string?, \"version\": string?, \"file\": string?, \"from\": string?, \"to\": string?}, \"confidence\": <float 0..1>, \"risk\": <float 0..1>}. Allowed operators ONLY: 1 = DEPENDENCY_REPAIR (bump/replace a failing dependency in package.json; params dependency, version), 9 = VERSION_PIN (pin an exact version; params dependency, version), 2 = SYNTAX_FIX (small textual fix in one file; params file, from, to), 8 = IMPORT_PATH_FIX (fix a broken import path; params file, from, to), 12 = SOURCE_REPAIR (small bounded find/replace edit in one source file; params file, from, to). Edits are STRICTLY bounded: from/to must be short exact literal substrings already present in the named file; never whole-file rewrites, never new files, never regex. If the incident is not actionable, reply {\"operator\": 0, \"parameters\": {}, \"confidence\": 0.0, \"risk\": 1.0}.";

/// Operadores de edicion acotada de codigo que el LLM puede proponer
/// (2 SYNTAX_FIX, 8 IMPORT_PATH_FIX, 12 SOURCE_REPAIR). El LLM NO escribe
/// codigo libre: solo nombra `file`/`from`/`to` y el operador DETERMINISTA
/// (repair_operators::diff) aplica el find/replace, valida el ancla unica
/// y respeta MAX_FILE_BYTES.
const EDIT_OPERATORS: [OperatorId; 3] = [
    OperatorId::SyntaxFix,
    OperatorId::ImportPathFix,
    OperatorId::SourceRepair,
];

/// Propone una accion estructurada. None = fail-closed (escalacion humana).
#[worker::send]
pub async fn propose(
    env: &Env,
    incident: &Incident,
    signature: &FailureSignature,
) -> Option<RepairAction> {
    if env.var(VAR_ENABLED).map(|v| v.to_string()).ok().as_deref() != Some("true") {
        console_warn!("llm fallback disabled: LLM_ENABLED != true");
        return None;
    }
    if !reserve_budget(env).await {
        console_warn!("llm budget exhausted: fail-closed, escalando a humano");
        return None;
    }
    let model = env
        .var(VAR_MODEL)
        .map(|v| v.to_string())
        .ok()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let request = serde_json::json!({
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": build_prompt(incident, signature) }
        ],
        "max_tokens": 512,
        "temperature": 0.2
    });
    let ai = env.ai(AI_BINDING).ok()?;
    let stream = ai.run_bytes(&model, &request).await.ok()?;
    let mut response = worker::Response::from_stream(stream).ok()?;
    let body = response.text().await.ok()?;
    action_from_response_body(&body)
}

/// Contador diario de presupuesto. Fail-closed: sin KV no hay LLM.
#[worker::send]
async fn reserve_budget(env: &Env) -> bool {
    let cap = env
        .var(VAR_DAILY_BUDGET)
        .map(|v| v.to_string())
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_DAILY_BUDGET);
    let cost = env
        .var(VAR_COST_PER_CALL)
        .map(|v| v.to_string())
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COST_PER_CALL);
    let Ok(kv) = env.kv(CONFIG_KV) else {
        return false;
    };
    let today = utc_date_from_unix_ms(crate::runtime::now_ms());
    let stored = kv
        .get(KEY_BUDGET)
        .text()
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Budget>(&raw).ok())
        .unwrap_or_default();
    let mut budget = if stored.date == today {
        stored
    } else {
        Budget::default()
    };
    if !budget_allows(&budget, cap, cost) {
        return false;
    }
    budget.date = today;
    budget.spent += cost;
    match serde_json::to_string(&budget) {
        Ok(serialized) => match kv.put(KEY_BUDGET, serialized) {
            Ok(builder) => builder.execute().await.is_ok(),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Presupuesto del dia: puro y testeado. El cambio de fecha UTC reinicia
/// el contador (el free tier se renueva a las 00:00 UTC).
pub fn budget_allows(budget: &Budget, cap: i64, cost: i64) -> bool {
    budget.spent + cost <= cap
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Budget {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub spent: i64,
}

/// Fecha civil UTC (algoritmo de Howard Hinnant): pura y testeable, sin
/// depender del locale ni del reloj del runtime mas alla del epoch ms.
pub fn utc_date_from_unix_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe.div_euclid(1460) + doe.div_euclid(36_524) - doe.div_euclid(146_096)) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe.div_euclid(4) - yoe.div_euclid(100));
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Prompt de usuario: incidente + firma, SIN codigo fuente del repo (el LLM
/// jamas ve el arbol; solo el contexto acotado del incidente).
pub fn build_prompt(incident: &Incident, signature: &FailureSignature) -> String {
    format!(
        "Incident:\n- error_code: {}\n- error_step: {}\n- command: {}\n- message: {}\n- language_hint: {}\n- framework_hint: {}\n- signature: {}\n\nPropose the repair action JSON.",
        incident.error_code,
        incident.error_step,
        incident.command,
        incident.message,
        incident.language_hint,
        incident.framework_hint,
        signature.fingerprint
    )
}

/// Extrae el primer objeto JSON balanceado por llaves de un texto libre.
pub fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end > start {
        Some(&raw[start..=end])
    } else {
        None
    }
}

/// Salida del modelo -> RepairAction validada. Operador no accionable o
/// valores no finitos => None (fail-closed: nunca un parche inventado).
pub fn action_from_response_body(body: &str) -> Option<RepairAction> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    // env.AI.run devuelve {"response": "..."} para modelos de texto.
    let candidate = value
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or(body);
    let json = extract_json_object(candidate)?;
    let proposal: Proposal = serde_json::from_str(json).ok()?;
    proposal.to_action()
}

#[derive(Debug, Deserialize)]
struct Proposal {
    operator: u8,
    #[serde(default)]
    parameters: std::collections::BTreeMap<String, String>,
    confidence: f32,
    risk: f32,
}

impl Proposal {
    fn to_action(&self) -> Option<RepairAction> {
        let operator = OperatorId::from_u8(self.operator);
        if matches!(operator, OperatorId::NoOp | OperatorId::Unknown) {
            return None;
        }
        let confidence = clamp01(self.confidence)?;
        let risk = clamp01(self.risk)?;
        let mut parameters = self.parameters.clone();
        if EDIT_OPERATORS.contains(&operator) {
            // Edicion acotada: sin archivo o sin ancla from->to no hay
            // parche seguro. Fail-closed, nunca un edit inventado.
            let file = parameters.get("file")?;
            let from = parameters.get("from")?;
            let to = parameters.get("to")?;
            if file.is_empty() || from.is_empty() || from == to {
                return None;
            }
            // El ejecutor determinista solo entiende file/from/to.
            parameters.retain(|key, _| matches!(key.as_str(), "file" | "from" | "to"));
        }
        Some(RepairAction {
            node_id: String::from("llm:fallback"),
            repair_operator: operator,
            parameters,
            confidence,
            risk,
        })
    }
}

fn clamp01(value: f32) -> Option<f32> {
    if value.is_finite() {
        Some(value.clamp(0.0, 1.0))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_date_epoch_and_day_increment() {
        assert_eq!(utc_date_from_unix_ms(0), "1970-01-01");
        assert_eq!(utc_date_from_unix_ms(86_400_000), "1970-01-02");
        assert_eq!(utc_date_from_unix_ms(-1), "1969-12-31");
    }

    #[test]
    fn utc_date_handles_leap_day() {
        // 1972-02-29 = 789 dias despues del epoch.
        assert_eq!(utc_date_from_unix_ms(789 * 86_400_000), "1972-02-29");
        assert_eq!(utc_date_from_unix_ms(790 * 86_400_000), "1972-03-01");
    }

    #[test]
    fn budget_allows_respects_cap() {
        let b = |spent: i64| Budget {
            date: String::from("2026-10-08"),
            spent,
        };
        assert!(budget_allows(&b(7_700), 8_000, 300));
        assert!(!budget_allows(&b(7_700), 8_000, 301));
        assert!(!budget_allows(&b(8_000), 8_000, 1));
    }

    #[test]
    fn budget_default_is_zero() {
        assert!(budget_allows(&Budget::default(), 8_000, 300));
    }

    #[test]
    fn extract_json_finds_object_in_prose() {
        let raw = "Sure! Here is the action: {\"operator\": 1} hope it helps";
        assert_eq!(extract_json_object(raw), Some("{\"operator\": 1}"));
        assert_eq!(extract_json_object("no json"), None);
        assert_eq!(extract_json_object("{"), None);
    }

    #[test]
    fn action_from_wrapped_response() {
        let inner = "{\\\"operator\\\": 1, \\\"parameters\\\": {}, \\\"confidence\\\": 0.8, \\\"risk\\\": 0.2}";
        let plain = format!("{{\"response\": \"{inner}\"}}");
        let action = action_from_response_body(&plain).expect("accion valida");
        assert_eq!(action.node_id, "llm:fallback");
        assert_eq!(action.repair_operator, OperatorId::DependencyRepair);
        assert!((action.confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn action_rejects_noop_and_nonfinite() {
        let noop = "{\"operator\": 0, \"parameters\": {}, \"confidence\": 0.9, \"risk\": 0.1}";
        assert!(action_from_response_body(noop).is_none());
        let unknown = "{\"operator\": 42, \"parameters\": {}, \"confidence\": 0.9, \"risk\": 0.1}";
        assert!(action_from_response_body(unknown).is_none());
        assert!(clamp01(f32::NAN).is_none());
        assert!(clamp01(f32::INFINITY).is_none());
        assert_eq!(clamp01(1.7), Some(1.0));
        assert_eq!(clamp01(-0.5), Some(0.0));
    }

    #[test]
    fn action_accepts_bounded_source_repair() {
        let body = "{\"operator\": 12, \"parameters\": {\"file\": \"src/main.ts\", \"from\": \"../old/path\", \"to\": \"../new/path\"}, \"confidence\": 0.9, \"risk\": 0.2}";
        let action = action_from_response_body(body).expect("edicion acotada valida");
        assert_eq!(action.repair_operator, OperatorId::SourceRepair);
        assert_eq!(action.parameters.len(), 3);
        assert_eq!(action.parameters.get("file").map(String::as_str), Some("src/main.ts"));
    }

    #[test]
    fn action_rejects_invalid_edits() {
        // from == to: parche nulo.
        let same = "{\"operator\": 2, \"parameters\": {\"file\": \"a.ts\", \"from\": \"x\", \"to\": \"x\"}, \"confidence\": 0.9, \"risk\": 0.2}";
        assert!(action_from_response_body(same).is_none());
        // Sin ancla from (vacia): nada seguro que buscar.
        let no_from = "{\"operator\": 2, \"parameters\": {\"file\": \"a.ts\", \"from\": \"\", \"to\": \"y\"}, \"confidence\": 0.9, \"risk\": 0.2}";
        assert!(action_from_response_body(no_from).is_none());
        // Sin archivo: sin target acotado.
        let no_file = "{\"operator\": 8, \"parameters\": {\"from\": \"a\", \"to\": \"b\"}, \"confidence\": 0.9, \"risk\": 0.2}";
        assert!(action_from_response_body(no_file).is_none());
    }

    #[test]
    fn prompt_contains_incident_context() {
        let incident = Incident {
            id: String::from("inc-1"),
            source: String::from("webhook"),
            error_code: String::from("E404"),
            error_step: String::from("install"),
            command: String::from("npm install"),
            message: String::from("not found"),
            project: String::from("acme/api"),
            attempts: 1,
            stack_hint: String::new(),
            language_hint: String::from("js"),
            framework_hint: String::new(),
            verified: false,
            status: String::from("open"),
        };
        let signature = FailureSignature::from_incident(&incident);
        let prompt = build_prompt(&incident, &signature);
        assert!(prompt.contains("E404"));
        assert!(prompt.contains("npm install"));
    }
}

