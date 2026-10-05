//! Consumidor de la cola de reparaciones.
//!
//! Reglas PART3: retry acotado (nunca infinito: max_retries=3 de la cola ->
//! DLQ), deteccion de veneno en el consumidor de la DLQ, correlation_id en
//! cada mensaje e idempotencia via el Durable Object (la puerta /attempt
//! decide si el intento procede; un duplicado nunca consume quota dos veces).

use serde::{Deserialize, Serialize};
use worker::*;

use feature_engine::extract;
use repair_operators::gate;
use repair_types::{FailureSignature, Incident};

use crate::runtime::{
    anti_loop::AntiLoopConfig,
    model,
    quota::QuotaConfig,
    rules,
    security::{fnv1a64, urlencode},
    MAX_RISK, MIN_CONFIDENCE,
};

pub const QUEUE_BINDING: &str = "REPAIR_QUEUE";
pub const QUEUE_PROD: &str = "auto-healing-repairs";
pub const QUEUE_STAGING: &str = "auto-healing-repairs-staging";
pub const DLQ_PROD: &str = "auto-healing-repairs-dlq";
pub const DLQ_STAGING: &str = "auto-healing-repairs-dlq-staging";

/// Delay fijo de reintento. Antes habia un "contador propio" (MAX_QUEUE_ATTEMPTS)
/// que era inalcanzable: Cloudflare reentrega el MISMO body al reintentar, asi
/// que el contador del mensaje nunca avanzaba y la rama nunca se ejecutaba
/// (dead code). El hard stop autoritativo de intentos es el DO (`/attempt`,
/// max_attempts_per_incident) y el tope de reintentos es `max_retries=3` de la
/// cola (trascendido, el mensaje cae a la DLQ). Nunca retry infinito.
const RETRY_DELAY_SECONDS: u32 = 10;

/// Cuerpo entrante del webhook. Todo opcional: un payload parcial nunca
/// rompe el isolate.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub error_code: String,
    #[serde(default)]
    pub error_step: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub stack_hint: String,
    #[serde(default)]
    pub language_hint: String,
    #[serde(default)]
    pub framework_hint: String,
}

impl WebhookPayload {
    pub fn incident_id(&self) -> String {
        if self.id.is_empty() {
            "unidentified".to_string()
        } else {
            self.id.clone()
        }
    }

    pub fn repo(&self) -> String {
        if self.project.is_empty() {
            "default".to_string()
        } else {
            self.project.clone()
        }
    }

    /// Firma canonica para dedup/anti-loop (determinista, sin reloj).
    pub fn signature_str(&self) -> String {
        format!("{}|{}|{}", self.error_code, self.error_step, self.command)
    }

    pub fn to_incident(&self) -> Incident {
        Incident {
            id: self.incident_id(),
            source: if self.source.is_empty() {
                "webhook".to_string()
            } else {
                self.source.clone()
            },
            error_code: self.error_code.clone(),
            error_step: self.error_step.clone(),
            command: self.command.clone(),
            message: self.message.clone(),
            project: self.project.clone(),
            attempts: self.attempts,
            stack_hint: self.stack_hint.clone(),
            language_hint: self.language_hint.clone(),
            framework_hint: self.framework_hint.clone(),
            verified: false,
            status: "open".to_string(),
        }
    }
}

/// Mensaje de cola: correlation_id SIEMPRE presente para trazabilidad.
///
/// Sin campo `attempts`: el body del mensaje es inmutable al reintentar,
/// asi que un contador aqui no sobreviviria a los redeliveries. El numero de
/// intentos REALES vive en el DO (columna `incidents.attempts`, la incrementa
/// `/attempt`); ver `RETRY_DELAY_SECONDS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub correlation_id: String,
    pub incident_id: String,
    pub repo: String,
    pub signature: String,
    pub payload: WebhookPayload,
    pub enqueued_at: i64,
}

/// Punto de entrada del evento de cola. Un solo handler para produccion,
/// staging y DLQ: se despacha por nombre de cola.
#[worker::send]
pub async fn consume(batch: MessageBatch<QueueTask>, env: Env) -> Result<()> {
    let queue_name = batch.queue();
    if queue_name == DLQ_PROD || queue_name == DLQ_STAGING {
        // DLQ: registrar el veneno en el DO (best-effort) y asentar. El
        // nombre de la cola REAL va al DO: un veneno de staging no se
        // registra como produccion.
        for message in batch.messages()? {
            let task = message.body().clone();
            if let Err(e) = record_poison(env.clone(), task, &queue_name).await {
                console_error!("dlq record failed: {}", e);
            }
        }
        batch.ack_all();
        return Ok(());
    }
    if queue_name != QUEUE_PROD && queue_name != QUEUE_STAGING {
        console_error!("unknown queue: {}", queue_name);
        return Ok(());
    }

    let quota_cfg = QuotaConfig::from_env(&env);
    let anti_cfg = AntiLoopConfig::from_env(&env);
    for message in batch.messages()? {
        let task = message.body().clone();
        match process(env.clone(), task.clone(), quota_cfg, anti_cfg).await {
            Ok(()) => {
                message.ack();
            }
            Err(err) => {
                // Fallo transitorio (DO o KV no disponible). Se reintenta con
                // delay fijo; el TOPE es max_retries=3 de la cola, y ahi el
                // mensaje cae a la DLQ (su consumidor lo registra como
                // veneno). El corte autoritativo por numero de intentos es el
                // DO (/attempt), no este consumidor.
                console_warn!(
                    "retry correlation_id={} en {}s: {}",
                    task.correlation_id,
                    RETRY_DELAY_SECONDS,
                    err
                );
                message.retry_with_options(
                    &QueueRetryOptionsBuilder::new()
                        .with_delay_seconds(RETRY_DELAY_SECONDS)
                        .build(),
                );
            }
        }
    }
    Ok(())
}

/// Procesa un intento: puerta DO -> pipeline -> registro de resultado.
#[worker::send]
async fn process(
    env: Env,
    task: QueueTask,
    quota_cfg: QuotaConfig,
    anti_cfg: AntiLoopConfig,
) -> Result<()> {
    // 1. Puerta: quota + anti-loop (las 4 señales) + contador de attempts en el DO.
    let qs = format!(
        "/attempt?correlation_id={}&incident_id={}&signature={}&max_attempts_per_incident={}&max_same_signature={}&max_same_fingerprint={}&max_same_failing_verification={}&window_seconds={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        urlencode(&task.signature),
        quota_cfg.max_attempts_per_incident,
        anti_cfg.max_same_signature,
        anti_cfg.max_same_fingerprint,
        anti_cfg.max_same_failing_verification,
        anti_cfg.window_seconds
    );
    let text = crate::runtime::call_do(env.clone(), task.repo.clone(), qs).await?;
    let verdict: serde_json::Value = serde_json::from_str(&text)?;
    if !verdict
        .get("allowed")
        .and_then(|a| a.as_bool())
        .unwrap_or(false)
    {
        // Bloqueado por quota/anti-loop/hard-stop (ya registrado en el DO).
        return Ok(());
    }

    // 2. Pipeline determinista: Incident -> features -> NN -> gate.
    let incident = task.payload.to_incident();
    let signature = FailureSignature::from_incident(&incident);
    let features = extract(&incident, &signature);

    // 2.5 Reglas declarativas (P1): filtrado adicional fail-closed.
    // REPAIR_RULES invalido => bloqueado (nunca dejar pasar por defecto);
    // sin var o "[]" => sin reglas (no-op). Esquema y sintaxis: rules.rs.
    let ruleset = match rules::load(&env) {
        Ok(r) => r,
        Err(e) => {
            console_error!("rules config invalid: {}", e);
            let rqs = blocked_result_qs(&task, "blocked_rules", "rules_invalid_config");
            crate::runtime::call_do(env, task.repo.clone(), rqs).await?;
            return Ok(());
        }
    };

    let loaded = match model::load(&env).await {
        Ok(m) => m,
        Err(_) => {
            // current y stable ausentes o malformados: BLOCKED. Nunca zeros.
            let rqs = blocked_result_qs(&task, "blocked_model", "no_model_current_stable");
            crate::runtime::call_do(env, task.repo.clone(), rqs).await?;
            return Ok(());
        }
    };
    let action = loaded.net.predict(&features);
    let gate_ok = gate(&action, MIN_CONFIDENCE, MAX_RISK).is_ok();

    // 2.6 Evaluacion declarativa: las reglas solo restringen (block) u
    // observan (observe); NUNCA permiten saltarse el gate determinista
    // (autoridad: repair_operators::gate + VERIFY en GitHub Actions).
    let rule_ctx = rules::from_incident_and_action(
        &incident,
        &task.repo,
        &task.signature,
        action.repair_operator.as_str(),
        action.confidence,
        action.risk,
    );
    match ruleset.evaluate(&rule_ctx) {
        Ok((Some(rule_id), observed)) => {
            if !observed.is_empty() {
                console_log!("rules observed: {}", observed.join(","));
            }
            console_warn!("rule blocked: {}", rule_id);
            let reason = format!("rule:{}", rule_id);
            let rqs = blocked_result_qs(&task, "blocked_by_rule", &reason);
            crate::runtime::call_do(env, task.repo, rqs).await?;
            return Ok(());
        }
        Ok((None, observed)) => {
            if !observed.is_empty() {
                console_log!("rules observed: {}", observed.join(","));
            }
        }
        Err(e) => {
            // No deberia ocurrir (compile valida campos y tipos); el
            // path sigue fail-closed: nunca unwrap().
            console_error!("rules eval error: {}", e);
            let rqs = blocked_result_qs(&task, "blocked_rules", "rules_eval_error");
            crate::runtime::call_do(env, task.repo, rqs).await?;
            return Ok(());
        }
    }

    let decision = if gate_ok {
        "allow"
    } else {
        "blocked_by_policy"
    };
    let fingerprint = format!(
        "{:x}",
        fnv1a64(format!("{}|{}", action.repair_operator as u8, task.signature).as_bytes())
    );

    // 3. Registrar decision + verificacion (pending_ci: VERIFY = Actions).
    let verify_status = if gate_ok { "pending_ci" } else { "blocked" };
    let rqs = format!(
        "/result?correlation_id={}&incident_id={}&decision={}&fingerprint={}&verify_status={}&evidence_ref={}&reason=",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        decision,
        urlencode(&fingerprint),
        verify_status,
        "github_actions"
    );
    crate::runtime::call_do(env, task.repo, rqs).await?;
    Ok(())
}

fn blocked_result_qs(task: &QueueTask, decision: &str, reason: &str) -> String {
    format!(
        "/result?correlation_id={}&incident_id={}&decision={}&fingerprint=&verify_status=blocked&evidence_ref=&reason={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        decision,
        urlencode(reason)
    )
}

#[worker::send]
async fn record_poison(env: Env, task: QueueTask, queue_name: &str) -> Result<()> {
    let qs = format!(
        "/poison?correlation_id={}&incident_id={}&queue={}",
        urlencode(&task.correlation_id),
        urlencode(&task.incident_id),
        urlencode(queue_name)
    );
    crate::runtime::call_do(env, task.repo, qs)
        .await
        .map(|_| ())
}
